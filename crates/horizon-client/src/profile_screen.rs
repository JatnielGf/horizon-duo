//! The state of the profile picker: which profile is selected, the name being
//! typed, the delete confirmation, and what each button asks for. It draws
//! nothing and starts nothing; the title screen shows it and acts on its
//! [`Outcome`], as it does for `multiplayer::Screen`.
use crate::hud::{HudAtlas, HudBatch, rgba};
use crate::profile::{self, Profile, Profiles};

/// A button: x, y, width, label, enabled, id (the shape `multiplayer` uses).
pub type ButtonSpec = (f32, f32, f32, String, bool, u8);

pub const USE: u8 = 0;
pub const NEW: u8 = 1;
pub const RENAME: u8 = 2;
pub const DELETE: u8 = 3;
pub const BACK: u8 = 4;
pub const SAVE: u8 = 5;
pub const CANCEL: u8 = 6;
pub const CONFIRM: u8 = 7;

/// The list's rows: where it starts and how tall each is.
const LIST_TOP: f32 = 36.0;
const ROW_HEIGHT: f32 = 20.0;
/// The space under the list: the message line and the buttons.
const LIST_BOTTOM_GAP: f32 = 142.0;
const ROW_WIDTH: f32 = 200.0;

/// What a click asks the title screen to do.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    /// Leave the picker.
    Back,
    /// Start playing in the profile with this id (it is already active in the
    /// index; the caller moves the game into its folder).
    Switch(String),
}

#[derive(Clone, Debug)]
pub enum Screen {
    List {
        profiles: Vec<Profile>,
        selected: usize,
        /// The profile this run of the game is in.
        running: String,
        /// The profile the game starts in (the one chosen with Use).
        active: String,
        /// Why the last button did nothing (a folder that could not be made).
        error: Option<String>,
    },
    /// A new profile (`id` None) or a new name for one.
    Edit {
        id: Option<String>,
        name: String,
        error: Option<String>,
        running: String,
    },
    ConfirmDelete {
        id: String,
        name: String,
        running: String,
    },
}

impl Screen {
    /// The picker, with the profile the game starts in selected. `running` is
    /// the profile this run is in.
    pub fn list(profiles: &Profiles, running: &str) -> Self {
        let all = profiles.list();
        let active = profiles.active().id;
        let selected = all.iter().position(|p| p.id == active).unwrap_or(0);
        Self::List {
            profiles: all,
            selected,
            running: running.to_owned(),
            active,
            error: None,
        }
    }

    /// Buttons for a window of `w` by `h`, in the picker's column.
    pub fn buttons(&self, w: f32, h: f32) -> Vec<ButtonSpec> {
        let x = w / 2.0 - 100.0;
        let bottom = h - 52.0;
        let text = |key: &str| key.to_owned();
        match self {
            Self::List {
                profiles,
                selected,
                running,
                active,
                ..
            } => {
                let chosen = profiles.get(*selected);
                let custom = chosen.is_some_and(|p| !p.is_default());
                let in_use = chosen.is_some_and(|p| p.id == *active);
                let busy = chosen.is_some_and(|p| p.id == *active || p.id == *running);
                vec![
                    (
                        x,
                        bottom - 72.0,
                        98.0,
                        text("Use"),
                        chosen.is_some() && !in_use,
                        USE,
                    ),
                    (x + 102.0, bottom - 72.0, 98.0, text("New"), true, NEW),
                    (x, bottom - 48.0, 98.0, text("Rename"), custom, RENAME),
                    (
                        x + 102.0,
                        bottom - 48.0,
                        98.0,
                        text("Delete"),
                        custom && !busy,
                        DELETE,
                    ),
                    (x, bottom - 24.0, 200.0, text("Back"), true, BACK),
                ]
            }
            Self::Edit { name, .. } => vec![
                (
                    x,
                    bottom - 24.0,
                    98.0,
                    text("Save"),
                    profile::clean_name(name).is_some(),
                    SAVE,
                ),
                (x + 102.0, bottom - 24.0, 98.0, text("Cancel"), true, CANCEL),
            ],
            Self::ConfirmDelete { .. } => vec![
                (x, bottom - 24.0, 98.0, text("Delete"), true, CONFIRM),
                (x + 102.0, bottom - 24.0, 98.0, text("Cancel"), true, CANCEL),
            ],
        }
    }

    /// Selects the row at index `row` of the list, if there is one.
    pub fn select(&mut self, row: usize) {
        if let Self::List {
            profiles, selected, ..
        } = self
            && row < profiles.len()
        {
            *selected = row;
        }
    }

    /// A button was pressed.
    pub fn click(&mut self, id: u8, profiles: &mut Profiles) -> Outcome {
        match (&mut *self, id) {
            (Self::List { .. }, BACK) => Outcome::Back,
            (
                Self::List {
                    profiles: all,
                    selected,
                    active,
                    error,
                    ..
                },
                USE,
            ) => {
                let Some(target) = all.get(*selected).cloned() else {
                    return Outcome::None;
                };
                if target.id == *active {
                    return Outcome::None;
                }
                match profiles.set_active(&target.id) {
                    Ok(()) => {
                        *active = target.id.clone();
                        *error = None;
                        Outcome::Switch(target.id)
                    }
                    Err(e) => {
                        *error = Some(e.to_string());
                        Outcome::None
                    }
                }
            }
            (Self::List { running, .. }, NEW) => {
                *self = Self::Edit {
                    id: None,
                    name: String::new(),
                    error: None,
                    running: running.clone(),
                };
                Outcome::None
            }
            (
                Self::List {
                    profiles: all,
                    selected,
                    running,
                    ..
                },
                RENAME,
            ) => {
                if let Some(p) = all.get(*selected).filter(|p| !p.is_default()) {
                    *self = Self::Edit {
                        id: Some(p.id.clone()),
                        name: p.name.clone(),
                        error: None,
                        running: running.clone(),
                    };
                }
                Outcome::None
            }
            (
                Self::List {
                    profiles: all,
                    selected,
                    running,
                    active,
                    ..
                },
                DELETE,
            ) => {
                if let Some(p) = all
                    .get(*selected)
                    .filter(|p| !p.is_default() && p.id != *active && p.id != *running)
                {
                    *self = Self::ConfirmDelete {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        running: running.clone(),
                    };
                }
                Outcome::None
            }
            (
                Self::Edit {
                    id, name, error, ..
                },
                SAVE,
            ) => {
                let saved = match id {
                    Some(id) => profiles.rename(id, name),
                    None => profiles.create(name).map(|_| ()),
                };
                match saved {
                    Ok(()) => self.back_to_list(profiles),
                    Err(e) => *error = Some(e.to_string()),
                }
                Outcome::None
            }
            (Self::ConfirmDelete { id, .. }, CONFIRM) => {
                let result = profiles.delete(id);
                self.back_to_list(profiles);
                if let (Err(e), Self::List { error, .. }) = (result, &mut *self) {
                    *error = Some(e.to_string());
                }
                Outcome::None
            }
            (Self::Edit { .. } | Self::ConfirmDelete { .. }, CANCEL) => {
                self.back_to_list(profiles);
                Outcome::None
            }
            _ => Outcome::None,
        }
    }

    /// Escape: one step back, and `true` when the picker itself should close.
    pub fn escape(&mut self, profiles: &Profiles) -> bool {
        match self {
            Self::List { .. } => true,
            Self::Edit { .. } | Self::ConfirmDelete { .. } => {
                self.back_to_list(profiles);
                false
            }
        }
    }

    /// The name being typed, when a name is.
    pub fn name_mut(&mut self) -> Option<&mut String> {
        match self {
            Self::Edit { name, error, .. } => {
                *error = None;
                Some(name)
            }
            _ => None,
        }
    }

    pub fn typing(&self) -> bool {
        matches!(self, Self::Edit { .. })
    }

    /// Typed characters, up to the longest name.
    pub fn type_text(&mut self, text: &str) {
        if let Some(name) = self.name_mut() {
            // A name in Minecraft: printable ASCII, no spaces.
            for c in text.chars().filter(char::is_ascii_graphic) {
                if name.chars().count() < profile::MAX_NAME_CHARS {
                    name.push(c);
                }
            }
        }
    }

    /// Empties the name being typed.
    pub fn clear_name(&mut self) {
        if let Some(name) = self.name_mut() {
            name.clear();
        }
    }

    pub fn backspace(&mut self) {
        if let Some(name) = self.name_mut() {
            name.pop();
        }
    }

    /// How many rows the list shows in a window `h` tall.
    fn capacity(h: f32) -> usize {
        (((h - LIST_BOTTOM_GAP - LIST_TOP) / ROW_HEIGHT)
            .floor()
            .max(1.0)) as usize
    }

    /// The first row shown, so that the selected one is in view.
    pub fn first_visible(selected: usize, capacity: usize, len: usize) -> usize {
        let first = (selected + 1).saturating_sub(capacity);
        first.min(len.saturating_sub(capacity))
    }

    /// The row under a click at (`x`, `y`), in a window of `w` by `h`.
    pub fn row_at(&self, x: f32, y: f32, (w, h): (f32, f32)) -> Option<usize> {
        let Self::List {
            profiles, selected, ..
        } = self
        else {
            return None;
        };
        let left = (w / 2.0).floor() - ROW_WIDTH / 2.0;
        let capacity = Self::capacity(h);
        let first = Self::first_visible(*selected, capacity, profiles.len());
        if x < left || x >= left + ROW_WIDTH || y < LIST_TOP {
            return None;
        }
        let row = ((y - LIST_TOP) / ROW_HEIGHT) as usize;
        (row < capacity && first + row < profiles.len()).then_some(first + row)
    }

    /// Draws the picker over the title's panorama; the buttons are drawn by the
    /// title, from [`Screen::buttons`].
    pub fn draw(&self, atlas: &HudAtlas, batch: &mut HudBatch, blink: bool, (w, h): (f32, f32)) {
        let cx = (w / 2.0).floor();
        let left = cx - ROW_WIDTH / 2.0;
        let white = u32::MAX;
        let gray = rgba(160, 160, 160, 255);
        let red = rgba(255, 85, 85, 255);
        let centered = |batch: &mut HudBatch, text: &str, y: f32, color: u32| {
            let width = HudBatch::text_width(atlas, text).ceil();
            batch.text(atlas, text, cx - (width / 2.0).floor(), y, color, true);
        };
        match self {
            Self::List {
                profiles,
                selected,
                running,
                active,
                error,
            } => {
                centered(batch, "Profiles", 14.0, white);
                let capacity = Self::capacity(h);
                let first = Self::first_visible(*selected, capacity, profiles.len());
                for (row, profile) in profiles.iter().skip(first).take(capacity).enumerate() {
                    let y = LIST_TOP + row as f32 * ROW_HEIGHT;
                    batch.fill(
                        atlas,
                        left,
                        y,
                        ROW_WIDTH,
                        ROW_HEIGHT - 2.0,
                        rgba(0, 0, 0, 140),
                    );
                    if first + row == *selected {
                        for (x, y, w, h) in [
                            (left, y, ROW_WIDTH, 1.0),
                            (left, y + ROW_HEIGHT - 3.0, ROW_WIDTH, 1.0),
                            (left, y, 1.0, ROW_HEIGHT - 2.0),
                            (left + ROW_WIDTH - 1.0, y, 1.0, ROW_HEIGHT - 2.0),
                        ] {
                            batch.fill(atlas, x, y, w, h, white);
                        }
                    }
                    batch.text(atlas, &profile.name, left + 6.0, y + 5.0, white, true);
                    let note = if profile.id == *active && active == running {
                        Some("in use")
                    } else if profile.id == *active {
                        Some("next launch")
                    } else if profile.id == *running {
                        Some("running")
                    } else {
                        None
                    };
                    if let Some(note) = note {
                        let width = HudBatch::text_width(atlas, note).ceil();
                        batch.text(
                            atlas,
                            note,
                            left + ROW_WIDTH - 6.0 - width,
                            y + 5.0,
                            gray,
                            true,
                        );
                    }
                }
                if let Some(error) = error {
                    centered(batch, error, h - LIST_BOTTOM_GAP + 4.0, red);
                }
            }
            Self::Edit {
                id, name, error, ..
            } => {
                centered(
                    batch,
                    if id.is_some() {
                        "New name"
                    } else {
                        "New profile"
                    },
                    36.0,
                    white,
                );
                // A text field for the touch layer: a tap on it asks for the keyboard.
                crate::widgets::target_as(
                    [left, 50.0, ROW_WIDTH, ROW_HEIGHT],
                    crate::widgets::Kind::Field,
                );
                batch.fill(atlas, left, 50.0, ROW_WIDTH, ROW_HEIGHT, white);
                batch.fill(
                    atlas,
                    left + 1.0,
                    51.0,
                    ROW_WIDTH - 2.0,
                    ROW_HEIGHT - 2.0,
                    rgba(0, 0, 0, 255),
                );
                batch.text(atlas, name, left + 5.0, 56.0, white, true);
                if blink {
                    let caret = left + 5.0 + HudBatch::text_width(atlas, name).ceil();
                    batch.fill(atlas, caret, 55.0, 1.0, 10.0, white);
                }
                centered(
                    batch,
                    "Your name in the game: up to 16 letters, numbers or symbols, no spaces",
                    76.0,
                    gray,
                );
                if let Some(error) = error {
                    centered(batch, error, 92.0, red);
                }
            }
            Self::ConfirmDelete { name, .. } => {
                centered(batch, "Delete this profile?", 50.0, white);
                centered(batch, name, 66.0, white);
                centered(batch, "Its worlds and settings will be lost.", 82.0, gray);
            }
        }
    }

    /// Back to the list, with the profile the game starts in selected.
    fn back_to_list(&mut self, profiles: &Profiles) {
        let running = match self {
            Self::List { running, .. }
            | Self::Edit { running, .. }
            | Self::ConfirmDelete { running, .. } => running.clone(),
        };
        *self = Self::list(profiles, &running);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU32, Ordering},
    };

    fn temp_root() -> PathBuf {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "horizon-profile-screen-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn enabled(screen: &Screen, id: u8) -> bool {
        screen
            .buttons(800.0, 600.0)
            .iter()
            .find(|b| b.5 == id)
            .is_some_and(|b| b.4)
    }

    #[test]
    fn the_list_scrolls_to_keep_the_selected_row_in_view() {
        assert_eq!(Screen::first_visible(0, 5, 3), 0);
        assert_eq!(Screen::first_visible(4, 5, 12), 0);
        assert_eq!(Screen::first_visible(5, 5, 12), 1);
        assert_eq!(Screen::first_visible(11, 5, 12), 7);
        assert_eq!(Screen::first_visible(0, 5, 0), 0);
    }

    #[test]
    fn rows_are_found_from_a_click() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        profiles.create("Ana").unwrap();
        let screen = Screen::list(&profiles, "default");
        let size = (800.0, 600.0);
        assert_eq!(screen.row_at(400.0, LIST_TOP + 2.0, size), Some(0));
        assert_eq!(
            screen.row_at(400.0, LIST_TOP + ROW_HEIGHT + 2.0, size),
            Some(1)
        );
        assert_eq!(
            screen.row_at(400.0, LIST_TOP + 2.0 * ROW_HEIGHT + 2.0, size),
            None
        );
        assert_eq!(
            screen.row_at(10.0, LIST_TOP + 2.0, size),
            None,
            "left of the rows"
        );
        assert_eq!(screen.row_at(400.0, 5.0, size), None, "above the rows");
    }

    #[test]
    fn a_chosen_profile_applies_at_the_next_launch_and_cannot_be_deleted_while_running() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        let ana = profiles.create("Ana").unwrap();
        // The game runs in Ana; the next launch is the built-in profile.
        let mut screen = Screen::list(&profiles, &ana.id);
        screen.select(1);
        assert!(!enabled(&screen, DELETE), "the running profile stays");
        assert!(enabled(&screen, USE), "chosen for the next launch");
        assert_eq!(
            screen.click(USE, &mut profiles),
            Outcome::Switch(ana.id.clone())
        );
        assert!(!enabled(&screen, USE));
        // Choosing the built-in profile again.
        screen.select(0);
        assert_eq!(
            screen.click(USE, &mut profiles),
            Outcome::Switch("default".into())
        );
        assert!(Profiles::load(&root).active().is_default());
        // Still running in Ana: it can't be deleted until the game moves out.
        screen.select(1);
        assert!(!enabled(&screen, DELETE));
        screen.click(DELETE, &mut profiles);
        assert!(matches!(screen, Screen::List { .. }));
    }

    fn type_name(screen: &mut Screen, text: &str) {
        screen.type_text(text);
    }

    #[test]
    fn the_default_profile_is_the_only_one_at_first() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        let screen = Screen::list(&profiles, "default");
        assert!(!enabled(&screen, USE), "already in use");
        assert!(enabled(&screen, NEW));
        assert!(!enabled(&screen, RENAME), "the built-in one keeps its name");
        assert!(!enabled(&screen, DELETE));
        let mut screen = screen;
        assert_eq!(screen.click(BACK, &mut profiles), Outcome::Back);
    }

    #[test]
    fn a_new_profile_is_named_saved_and_listed() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        let mut screen = Screen::list(&profiles, "default");
        screen.click(NEW, &mut profiles);
        assert!(screen.typing());
        assert!(!enabled(&screen, SAVE), "no name yet");
        type_name(&mut screen, "Ana");
        assert!(enabled(&screen, SAVE));
        assert_eq!(screen.click(SAVE, &mut profiles), Outcome::None);
        match &screen {
            Screen::List { profiles: all, .. } => {
                assert_eq!(all.len(), 2);
                assert_eq!(all[1].name, "Ana");
            }
            other => panic!("expected the list, got {other:?}"),
        }
        assert!(root.join("profiles/profile-1").is_dir());
    }

    #[test]
    fn a_taken_name_stays_on_the_edit_screen_with_the_reason() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        profiles.create("Ana").unwrap();
        let mut screen = Screen::list(&profiles, "default");
        screen.click(NEW, &mut profiles);
        type_name(&mut screen, "ana");
        screen.click(SAVE, &mut profiles);
        match &screen {
            Screen::Edit { error, .. } => assert!(error.is_some()),
            other => panic!("expected the edit screen, got {other:?}"),
        }
        // Typing again clears the message.
        screen.type_text("x");
        assert!(matches!(&screen, Screen::Edit { error: None, .. }));
    }

    #[test]
    fn using_a_profile_makes_it_active_and_asks_to_switch() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        let ana = profiles.create("Ana").unwrap();
        let mut screen = Screen::list(&profiles, "default");
        screen.select(1);
        assert!(enabled(&screen, USE));
        assert_eq!(
            screen.click(USE, &mut profiles),
            Outcome::Switch(ana.id.clone())
        );
        assert_eq!(Profiles::load(&root).active(), ana);
        assert!(!enabled(&screen, USE), "now in use");
        assert!(
            !enabled(&screen, DELETE),
            "the profile in use can't be deleted"
        );
        assert!(enabled(&screen, RENAME));
    }

    #[test]
    fn deleting_needs_a_confirmation() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        profiles.create("Ana").unwrap();
        let mut screen = Screen::list(&profiles, "default");
        screen.select(1);
        screen.click(DELETE, &mut profiles);
        assert!(matches!(screen, Screen::ConfirmDelete { .. }));
        // Cancel keeps it.
        screen.click(CANCEL, &mut profiles);
        assert_eq!(Profiles::load(&root).list().len(), 2);
        screen.select(1);
        screen.click(DELETE, &mut profiles);
        screen.click(CONFIRM, &mut profiles);
        assert_eq!(Profiles::load(&root).list().len(), 1);
        assert!(matches!(screen, Screen::List { .. }));
        assert!(!root.join("profiles/profile-1").exists());
    }

    #[test]
    fn rename_starts_from_the_current_name_and_escape_steps_back() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        profiles.create("Ana").unwrap();
        let mut screen = Screen::list(&profiles, "default");
        screen.select(1);
        screen.click(RENAME, &mut profiles);
        match &screen {
            Screen::Edit { id, name, .. } => {
                assert_eq!(id.as_deref(), Some("profile-1"));
                assert_eq!(name, "Ana");
            }
            other => panic!("expected the edit screen, got {other:?}"),
        }
        screen.backspace();
        screen.type_text("ita");
        screen.click(SAVE, &mut profiles);
        assert_eq!(Profiles::load(&root).list()[1].name, "Anita");
        // Escape: from a sub-screen back to the list, from the list out.
        screen.click(NEW, &mut profiles);
        assert!(!screen.escape(&profiles));
        assert!(screen.escape(&profiles));
    }

    #[test]
    fn typing_stops_at_the_longest_name_and_skips_control_characters() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        let mut screen = Screen::list(&profiles, "default");
        screen.click(NEW, &mut profiles);
        screen.type_text(&"x".repeat(100));
        screen.type_text("\n\t");
        match &screen {
            Screen::Edit { name, .. } => {
                assert_eq!(name.chars().count(), profile::MAX_NAME_CHARS);
            }
            other => panic!("expected the edit screen, got {other:?}"),
        }
    }

    #[test]
    fn typing_keeps_only_what_a_minecraft_name_can_have() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        let mut screen = Screen::list(&profiles, "default");
        screen.click(NEW, &mut profiles);
        screen.type_text("An a\u{f1}_1-");
        match &screen {
            Screen::Edit { name, .. } => assert_eq!(name, "Ana_1-"),
            other => panic!("expected the edit screen, got {other:?}"),
        }
    }
}
