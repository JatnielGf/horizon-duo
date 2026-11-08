//! How the player has changed the touch controls: where each one sits relative to its default,
//! and the sizes and opacities to come. It is kept in `touch-layout.json` in the game folder,
//! so each profile has its own.
//!
//! Positions are fractions of the screen, not pixels: a layout moved a tenth of the screen to
//! the right is that on an iPhone and on an iPad. A control the file doesn't mention stays where
//! Pocket Edition's layout puts it, so no file, or an empty one, changes nothing.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs, io,
    path::Path,
    sync::{PoisonError, RwLock},
};

/// The file in the game folder.
pub const FILE: &str = "touch-layout.json";
const VERSION: u32 = 1;
/// How far a control may be moved from its place, as a share of the screen.
const OFFSET_MAX: f32 = 1.0;
const SCALE_RANGE: (f32, f32) = (0.5, 2.0);
/// Fully transparent controls can't be found again.
const OPACITY_MIN: f32 = 0.1;

/// A control, or a group that moves as one, that the player can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Control {
    /// The D-pad, with the joystick's resting place.
    DPad,
    Jump,
    Pause,
    /// Escape, on a world's menus.
    Back,
    Shift,
    Chat,
    /// The row of key caps along the top-left corner.
    Caps,
}

impl Control {
    pub const ALL: [Control; 7] = [
        Control::DPad,
        Control::Jump,
        Control::Pause,
        Control::Back,
        Control::Shift,
        Control::Chat,
        Control::Caps,
    ];
    /// The name in the file.
    pub fn key(self) -> &'static str {
        match self {
            Control::DPad => "dpad",
            Control::Jump => "jump",
            Control::Pause => "pause",
            Control::Back => "back",
            Control::Shift => "shift",
            Control::Chat => "chat",
            Control::Caps => "caps",
        }
    }
    fn index(self) -> usize {
        self as usize
    }
    /// Whether it may be hidden. The D-pad moves the player and Pause and Back leave a menu, so
    /// they stay where a finger can find them.
    pub fn can_hide(self) -> bool {
        !matches!(self, Control::DPad | Control::Pause | Control::Back)
    }
}

/// What the player changed in one control.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Tweak {
    /// How far it is moved right and down from its place, as a share of the screen's width and
    /// height.
    pub dx: f32,
    pub dy: f32,
    /// Its size, as a multiple of the default.
    pub scale: f32,
    pub opacity: f32,
    pub hidden: bool,
}

impl Default for Tweak {
    fn default() -> Self {
        Self {
            dx: 0.0,
            dy: 0.0,
            scale: 1.0,
            opacity: 1.0,
            hidden: false,
        }
    }
}

impl Tweak {
    /// The same with every value in its range; one that isn't a number is the default.
    fn sanitized(self) -> Self {
        let or = |value: f32, default: f32| if value.is_finite() { value } else { default };
        Self {
            dx: or(self.dx, 0.0).clamp(-OFFSET_MAX, OFFSET_MAX),
            dy: or(self.dy, 0.0).clamp(-OFFSET_MAX, OFFSET_MAX),
            scale: or(self.scale, 1.0).clamp(SCALE_RANGE.0, SCALE_RANGE.1),
            opacity: or(self.opacity, 1.0).clamp(OPACITY_MIN, 1.0),
            hidden: self.hidden,
        }
    }
}

/// The parts of the screen the notch, the rounded corners and the system bars cover, in physical
/// pixels from each edge. They come from the window, not from the file.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub top: f32,
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TouchConfig {
    /// Applied to every control, on top of its own.
    pub opacity: f32,
    pub scale: f32,
    tweaks: [Tweak; Control::ALL.len()],
    pub insets: Insets,
}

impl Default for TouchConfig {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            scale: 1.0,
            tweaks: [Tweak::default(); Control::ALL.len()],
            insets: Insets::default(),
        }
    }
}

/// What the file holds.
#[derive(Serialize)]
struct Stored {
    version: u32,
    opacity: f32,
    scale: f32,
    controls: BTreeMap<String, Tweak>,
}

impl TouchConfig {
    pub fn tweak(&self, control: Control) -> Tweak {
        self.tweaks[control.index()]
    }

    /// Changes one control; the values are brought into their ranges, and a control that can't be
    /// hidden is not.
    pub fn set(&mut self, control: Control, tweak: Tweak) {
        let mut tweak = tweak.sanitized();
        tweak.hidden &= control.can_hide();
        self.tweaks[control.index()] = tweak;
    }

    /// Everything back to Pocket Edition's layout (the insets stay: they are the screen's).
    pub fn reset(&mut self) {
        *self = Self {
            insets: self.insets,
            ..Self::default()
        };
    }

    /// Whether anything differs from the default layout.
    pub fn is_default(&self) -> bool {
        let defaults = Self::default();
        self.opacity == defaults.opacity
            && self.scale == defaults.scale
            && self.tweaks == defaults.tweaks
    }

    /// The configuration in `text`: a file that can't be read is the default, and so is a control
    /// or a value that can't be read. An unknown control is ignored and a value out of range is
    /// brought into it.
    pub fn from_json(text: &str) -> Self {
        let file: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
        let number = |key: &str, default: f32| {
            let value = file.get(key).and_then(serde_json::Value::as_f64);
            let value = value.map_or(default, |v| v as f32);
            if value.is_finite() { value } else { default }
        };
        let mut config = Self {
            opacity: number("opacity", 1.0).clamp(OPACITY_MIN, 1.0),
            scale: number("scale", 1.0).clamp(SCALE_RANGE.0, SCALE_RANGE.1),
            ..Self::default()
        };
        if let Some(controls) = file.get("controls").and_then(serde_json::Value::as_object) {
            for control in Control::ALL {
                let tweak = controls
                    .get(control.key())
                    .and_then(|value| serde_json::from_value::<Tweak>(value.clone()).ok());
                if let Some(tweak) = tweak {
                    config.set(control, tweak);
                }
            }
        }
        config
    }

    /// The file's text: the controls that differ from the default.
    pub fn to_json(&self) -> String {
        let stored = Stored {
            version: VERSION,
            opacity: self.opacity,
            scale: self.scale,
            controls: Control::ALL
                .into_iter()
                .filter(|&c| self.tweak(c) != Tweak::default())
                .map(|c| (c.key().to_owned(), self.tweak(c)))
                .collect(),
        };
        serde_json::to_string_pretty(&stored).unwrap_or_default()
    }

    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .map(|text| Self::from_json(&text))
            .unwrap_or_default()
    }

    /// Writes the file through a temporary one, so a crash leaves the old.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, self.to_json())?;
        fs::rename(&tmp, path)
    }

    /// A number that changes with the configuration, to tell a layout from another.
    pub fn fingerprint(&self) -> u32 {
        let mut hash: u32 = 0x811c_9dc5;
        let mut feed = |bits: u32| {
            for byte in bits.to_le_bytes() {
                hash = (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193);
            }
        };
        feed(self.opacity.to_bits());
        feed(self.scale.to_bits());
        for t in &self.tweaks {
            for value in [t.dx, t.dy, t.scale, t.opacity] {
                feed(value.to_bits());
            }
            feed(u32::from(t.hidden));
        }
        for value in [
            self.insets.top,
            self.insets.left,
            self.insets.bottom,
            self.insets.right,
        ] {
            feed(value.to_bits());
        }
        hash
    }
}

/// The configuration of the profile the game is in, read once and kept.
static CURRENT: RwLock<Option<TouchConfig>> = RwLock::new(None);

/// The current configuration, read from the game folder the first time.
pub fn current() -> TouchConfig {
    if let Some(config) = *CURRENT.read().unwrap_or_else(PoisonError::into_inner) {
        return config;
    }
    let mut slot = CURRENT.write().unwrap_or_else(PoisonError::into_inner);
    *slot.get_or_insert_with(|| TouchConfig::load(Path::new(FILE)))
}

/// Makes `config` the current one. The insets, which come from the window, are kept.
pub fn set_current(config: TouchConfig) {
    let mut slot = CURRENT.write().unwrap_or_else(PoisonError::into_inner);
    let insets = slot.map_or(config.insets, |c| c.insets);
    *slot = Some(TouchConfig { insets, ..config });
}

/// The window's insets, as the screen has them now.
pub fn set_insets(insets: Insets) {
    let mut config = current();
    if config.insets != insets {
        config.insets = insets;
        *CURRENT.write().unwrap_or_else(PoisonError::into_inner) = Some(config);
    }
}

/// Forgets the configuration: the next one is read from the game folder, which is another when
/// the game moved to another profile.
pub fn reload() {
    let mut slot = CURRENT.write().unwrap_or_else(PoisonError::into_inner);
    let insets = slot.map(|c| c.insets).unwrap_or_default();
    let mut config = TouchConfig::load(Path::new(FILE));
    config.insets = insets;
    *slot = Some(config);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU32, Ordering},
    };

    fn temp_file() -> PathBuf {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "horizon-touch-config-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join(FILE)
    }

    #[test]
    fn no_file_is_the_default_layout() {
        let path = temp_file();
        assert!(TouchConfig::load(&path).is_default());
        fs::write(&path, "").unwrap();
        assert!(TouchConfig::load(&path).is_default());
        fs::write(&path, "{ not json").unwrap();
        assert!(TouchConfig::load(&path).is_default());
        fs::write(&path, "{}").unwrap();
        assert!(TouchConfig::load(&path).is_default());
    }

    #[test]
    fn a_saved_layout_is_read_back() {
        let path = temp_file();
        let mut config = TouchConfig {
            opacity: 0.6,
            ..TouchConfig::default()
        };
        config.set(
            Control::Jump,
            Tweak {
                dx: -0.1,
                dy: 0.05,
                scale: 1.25,
                opacity: 0.5,
                hidden: false,
            },
        );
        config.set(
            Control::Chat,
            Tweak {
                hidden: true,
                ..Tweak::default()
            },
        );
        config.save(&path).unwrap();
        let again = TouchConfig::load(&path);
        assert_eq!(again, config);
        assert!(!again.is_default());
        // Only what differs is written.
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"jump\"") && text.contains("\"chat\""));
        assert!(!text.contains("\"dpad\""));
    }

    #[test]
    fn values_out_of_range_and_unknown_controls_are_fixed_or_ignored() {
        let json = r#"{
            "version": 9,
            "opacity": 7,
            "scale": 0,
            "future": true,
            "controls": {
                "jump": {"dx": 40, "dy": -40, "scale": 100, "opacity": 0},
                "dpad": {"dx": null},
                "gyroscope": {"dx": 0.5}
            }
        }"#;
        let config = TouchConfig::from_json(json);
        assert_eq!(config.opacity, 1.0);
        assert_eq!(config.scale, SCALE_RANGE.0);
        let jump = config.tweak(Control::Jump);
        assert_eq!((jump.dx, jump.dy), (OFFSET_MAX, -OFFSET_MAX));
        assert_eq!((jump.scale, jump.opacity), (SCALE_RANGE.1, OPACITY_MIN));
        assert_eq!(config.tweak(Control::DPad), Tweak::default());
        assert!(
            Control::ALL
                .into_iter()
                .filter(|&c| c != Control::Jump)
                .all(|c| config.tweak(c) == Tweak::default())
        );
    }

    #[test]
    fn a_value_that_is_not_a_number_is_the_default() {
        let mut config = TouchConfig::default();
        config.set(
            Control::Pause,
            Tweak {
                dx: f32::NAN,
                dy: f32::INFINITY,
                scale: f32::NAN,
                opacity: f32::NAN,
                hidden: false,
            },
        );
        assert_eq!(config.tweak(Control::Pause), Tweak::default());
    }

    #[test]
    fn the_fingerprint_tells_layouts_apart_and_ignores_nothing_that_moves_a_control() {
        let base = TouchConfig::default();
        let mut moved = base;
        moved.set(
            Control::Back,
            Tweak {
                dx: 0.01,
                ..Tweak::default()
            },
        );
        let mut notched = base;
        notched.insets.left = 150.0;
        assert_eq!(base.fingerprint(), TouchConfig::default().fingerprint());
        assert_ne!(base.fingerprint(), moved.fingerprint());
        assert_ne!(base.fingerprint(), notched.fingerprint());
    }

    #[test]
    fn only_the_controls_that_can_be_left_out_are_hidden() {
        let hidden = Tweak {
            hidden: true,
            ..Tweak::default()
        };
        let mut config = TouchConfig::default();
        for control in Control::ALL {
            config.set(control, hidden);
            assert_eq!(
                config.tweak(control).hidden,
                control.can_hide(),
                "{control:?}"
            );
        }
        // A file can't hide them either.
        let all = r#"{"controls": {"dpad": {"hidden": true}, "pause": {"hidden": true},
            "back": {"hidden": true}, "jump": {"hidden": true}}}"#;
        let file = TouchConfig::from_json(all);
        assert!(!file.tweak(Control::DPad).hidden);
        assert!(!file.tweak(Control::Pause).hidden);
        assert!(!file.tweak(Control::Back).hidden);
        assert!(file.tweak(Control::Jump).hidden);
    }

    #[test]
    fn reset_keeps_the_screens_insets() {
        let mut config = TouchConfig::default();
        config.insets.bottom = 90.0;
        config.opacity = 0.3;
        config.reset();
        assert!(config.is_default());
        assert_eq!(config.insets.bottom, 90.0);
    }
}
