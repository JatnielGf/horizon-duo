//! Where the touch controls are, in physical pixels: Pocket Edition's layout, sized in points so
//! a button is as big under the thumb on an iPhone as on an iPad.

use super::config::{Control, TouchConfig};
use super::functions::{CONTROLS, Place};
use crate::hud::HudBatch;

/// x, y, width, height in physical pixels.
pub(super) type Rect = [f32; 4];

/// Physical pixels per art pixel for each point: buttons are drawn in pixel art, and an 18-pixel
/// button is then about 50 points, a comfortable thumb.
const UNIT_PER_POINT: f64 = 2.75;
/// Sizes in art pixels.
pub(super) const CELL: f32 = 18.0;
const PITCH: f32 = 20.0;
const JUMP: f32 = 22.0;
const SMALL: f32 = 14.0;
const MARGIN: f32 = 8.0;
/// A fingertip's reach above the point it touches.
const FINGER: f32 = 12.0;
/// The joystick's reach, and its knob's size.
pub(super) const REACH: f32 = 22.0;
pub(super) const KNOB: f32 = 14.0;
/// The joystick's share of the screen, from the left edge.
const STICK_SHARE: f32 = 0.4;
/// A key cap's least side in GUI pixels, which its keys are written in: two of the game font's
/// characters and a margin.
const CAP_TEXT: f32 = 16.0;
/// The Fn panel's tiles, in GUI pixels: two lines of the game font, as wide as its longest label.
const TILE: [f32; 2] = [100.0, 26.0];
const TILE_GAP: f32 = 4.0;
const PANEL_PADDING: f32 = 6.0;
const PANEL_COLUMNS: u8 = 2;
const PANEL_ROWS: u8 = 3;

/// A square of the D-pad: its arrows, the forward diagonals that show while forward is held,
/// and the middle (sneak, or down while flying).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Forward,
    Back,
    Left,
    Right,
    ForwardLeft,
    ForwardRight,
    Middle,
}

impl Cell {
    pub(super) const ALL: [Cell; 7] = [
        Cell::Forward,
        Cell::Back,
        Cell::Left,
        Cell::Right,
        Cell::ForwardLeft,
        Cell::ForwardRight,
        Cell::Middle,
    ];
    fn at(col: i32, row: i32) -> Option<Cell> {
        Some(match (col, row) {
            (0, 0) => Cell::ForwardLeft,
            (1, 0) => Cell::Forward,
            (2, 0) => Cell::ForwardRight,
            (0, 1) => Cell::Left,
            (1, 1) => Cell::Middle,
            (2, 1) => Cell::Right,
            (1, 2) => Cell::Back,
            _ => return None,
        })
    }
    fn grid(self) -> (f32, f32) {
        match self {
            Cell::ForwardLeft => (0.0, 0.0),
            Cell::Forward => (1.0, 0.0),
            Cell::ForwardRight => (2.0, 0.0),
            Cell::Left => (0.0, 1.0),
            Cell::Middle => (1.0, 1.0),
            Cell::Right => (2.0, 1.0),
            Cell::Back => (1.0, 2.0),
        }
    }
    /// The move vector: x right, y forward.
    pub(super) fn vector(self) -> [f32; 2] {
        const D: f32 = std::f32::consts::FRAC_1_SQRT_2;
        match self {
            Cell::Forward => [0.0, 1.0],
            Cell::Back => [0.0, -1.0],
            Cell::Left => [-1.0, 0.0],
            Cell::Right => [1.0, 0.0],
            Cell::ForwardLeft => [-D, D],
            Cell::ForwardRight => [D, D],
            Cell::Middle => [0.0, 0.0],
        }
    }
    pub(super) fn forward(self) -> bool {
        matches!(self, Cell::Forward | Cell::ForwardLeft | Cell::ForwardRight)
    }
    pub(super) fn diagonal(self) -> bool {
        matches!(self, Cell::ForwardLeft | Cell::ForwardRight)
    }
}

/// The buttons outside the D-pad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Jump,
    Inventory,
    Pause,
    /// Escape, on a world's menus.
    Back,
    /// The keyboard's Shift, in inventories: bottom left, under the left thumb.
    Shift,
    /// Opens chat, in play: left of Pause.
    Chat,
}

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    size: [f32; 2],
    /// What the player changed, and the screen's notch and system bars.
    config: TouchConfig,
    /// Physical pixels per GUI pixel (the HUD's) and per art pixel.
    pub(super) gui: f32,
    pub(super) unit: f32,
}

impl Layout {
    /// A screen of `width` x `height` physical pixels, `scale_factor` of them to a point.
    pub fn new(width: u32, height: u32, scale_factor: f64) -> Self {
        Self::with_config(width, height, scale_factor, TouchConfig::default())
    }
    /// The same, with the player's changes and the screen's insets.
    pub fn with_config(width: u32, height: u32, scale_factor: f64, config: TouchConfig) -> Self {
        Self {
            size: [width as f32, height as f32],
            config,
            gui: HudBatch::new([width, height]).scale,
            unit: (scale_factor * UNIT_PER_POINT).round().max(2.0) as f32,
        }
    }
    /// Where the controls and the GUI are, as an identity: a resize, another GUI scale or
    /// another display gives another, which moves what lies under each finger.
    pub fn placement(&self) -> [u32; 5] {
        let [width, height] = self.size;
        let [a, b, c, d] = [width, height, self.gui, self.unit].map(f32::to_bits);
        [a, b, c, d, self.config.fingerprint()]
    }
    fn u(&self, art: f32) -> f32 {
        art * self.unit
    }
    /// A margin from an edge: Pocket Edition's, or where the notch, the rounded corners or the
    /// system bars reach further.
    fn margin(&self, inset: f32) -> f32 {
        self.u(MARGIN).max(inset)
    }
    /// How far the player moved `control`, from the place `r` it rests in: its offset, held
    /// inside the screen's safe area as a whole.
    fn shift(&self, control: Control, r: Rect) -> [f32; 2] {
        let t = self.config.tweak(control);
        if t.dx == 0.0 && t.dy == 0.0 {
            return [0.0, 0.0];
        }
        let [w, h] = self.size;
        let inset = self.config.insets;
        let held = |at: f32, low: f32, high: f32| at.clamp(low, high.max(low));
        [
            held(r[0] + t.dx * w, inset.left, w - inset.right - r[2]) - r[0],
            held(r[1] + t.dy * h, inset.top, h - inset.bottom - r[3]) - r[1],
        ]
    }
    fn moved(&self, control: Control, r: Rect) -> Rect {
        let [dx, dy] = self.shift(control, r);
        [r[0] + dx, r[1] + dy, r[2], r[3]]
    }
    /// The hotbar (`Hud.extractItemHotbar`).
    pub(super) fn hotbar(&self) -> Rect {
        let w = (self.size[0] / self.gui).ceil() as i32;
        let h = (self.size[1] / self.gui).ceil() as i32;
        [
            (w / 2 - 91) as f32 * self.gui,
            (h - 22) as f32 * self.gui,
            182.0 * self.gui,
            22.0 * self.gui,
        ]
    }
    pub(super) fn hotbar_slot(&self, at: [f32; 2]) -> Option<u8> {
        let [x, y, w, _] = self.hotbar();
        ((x..x + w).contains(&at[0]) && at[1] >= y).then(|| {
            ((at[0] - x) / self.gui - 1.0)
                .div_euclid(20.0)
                .clamp(0.0, 8.0) as u8
        })
    }
    /// The D-pad's 3 x 3 squares in the bottom-left corner, raised above the hotbar where a
    /// narrow screen brings them together.
    pub(super) fn dpad(&self) -> Rect {
        let side = self.u(2.0 * PITCH + CELL);
        let x = self.margin(self.config.insets.left);
        let mut y = self.size[1] - self.margin(self.config.insets.bottom) - side;
        let [hx, hy, ..] = self.hotbar();
        if x + side > hx {
            y = y.min(hy - self.u(MARGIN) - side);
        }
        self.moved(Control::DPad, [x, y, side, side])
    }
    pub(super) fn cell(&self, cell: Cell) -> Rect {
        let [x, y, ..] = self.dpad();
        let (col, row) = cell.grid();
        let pitch = self.u(PITCH);
        [x + col * pitch, y + row * pitch, self.u(CELL), self.u(CELL)]
    }
    /// The D-pad square under `at`, if `at` is on the D-pad.
    pub(super) fn cell_at(&self, at: [f32; 2]) -> Option<Cell> {
        let [x, y, side, _] = self.dpad();
        if !(x..x + side).contains(&at[0]) || !(y..y + side).contains(&at[1]) {
            return None;
        }
        let pitch = self.u(PITCH);
        let col = (((at[0] - x) / pitch) as i32).min(2);
        let row = (((at[1] - y) / pitch) as i32).min(2);
        Cell::at(col, row)
    }
    pub(super) fn button(&self, button: Button) -> Rect {
        let rest = self.resting_button(button);
        let control = match button {
            Button::Jump => Control::Jump,
            Button::Pause => Control::Pause,
            Button::Back => Control::Back,
            Button::Shift => Control::Shift,
            Button::Chat => Control::Chat,
            // Glued to the hotbar, which is Pocket Edition's.
            Button::Inventory => return rest,
        };
        let moved = self.moved(control, rest);
        // Chat follows Pause, which can be anywhere: it stays in the safe area too.
        if button == Button::Chat {
            return self.kept(moved);
        }
        moved
    }
    /// `r`, moved the least that puts it inside the screen's safe area.
    fn kept(&self, r: Rect) -> Rect {
        let [w, h] = self.size;
        let inset = self.config.insets;
        let held = |at: f32, low: f32, high: f32| at.clamp(low, high.max(low));
        [
            held(r[0], inset.left, w - inset.right - r[2]),
            held(r[1], inset.top, h - inset.bottom - r[3]),
            r[2],
            r[3],
        ]
    }
    /// Where a button rests, before the player's changes.
    fn resting_button(&self, button: Button) -> Rect {
        let [w, h] = self.size;
        let inset = self.config.insets;
        match button {
            Button::Jump => {
                let side = self.u(JUMP);
                [
                    w - self.margin(inset.right) - side,
                    h - self.margin(inset.bottom) - side,
                    side,
                    side,
                ]
            }
            Button::Inventory => {
                let [x, y, width, height] = self.hotbar();
                [x + width + self.u(2.0), y, height, height]
            }
            Button::Pause => {
                let side = self.u(SMALL);
                [
                    w - self.margin(inset.right) - side,
                    self.margin(inset.top),
                    side,
                    side,
                ]
            }
            Button::Back => {
                let side = self.u(SMALL);
                [self.margin(inset.left), self.margin(inset.top), side, side]
            }
            Button::Shift => {
                let side = self.u(CELL);
                [
                    self.margin(inset.left),
                    h - self.margin(inset.bottom) - side,
                    side,
                    side,
                ]
            }
            Button::Chat => {
                let side = self.u(SMALL);
                let pause = self.button(Button::Pause);
                let left = pause[0] - self.u(2.0) - side;
                // Left of Pause, or right of it where there is no room on its left.
                let x = if left >= inset.left {
                    left
                } else {
                    pause[0] + pause[2] + self.u(2.0)
                };
                [x, self.margin(inset.top), side, side]
            }
        }
    }
    pub(super) fn on(&self, button: Button, at: [f32; 2]) -> bool {
        contains(self.button(button), at)
    }
    /// A key cap in a row along the top-left corner, or a tile of the panel.
    pub(super) fn control(&self, place: Place) -> Rect {
        match place {
            Place::Cap(i) => {
                let [x, y, side, _] = self.cap_at_rest(i);
                let [dx, dy] = self.caps_shift();
                [x + dx, y + dy, side, side]
            }
            Place::Tile(i) => {
                let [x, y, ..] = self.panel();
                let (col, row) = (f32::from(i % PANEL_COLUMNS), f32::from(i / PANEL_COLUMNS));
                let [w, h] = TILE;
                [
                    x + (PANEL_PADDING + col * (w + TILE_GAP)) * self.gui,
                    y + (PANEL_PADDING + row * (h + TILE_GAP)) * self.gui,
                    w * self.gui,
                    h * self.gui,
                ]
            }
        }
    }
    /// Where key cap `i` rests, before the player's changes.
    fn cap_at_rest(&self, i: u8) -> Rect {
        let inset = self.config.insets;
        let side = self.u(CELL).max(CAP_TEXT * self.gui);
        let x = self.margin(inset.left) + f32::from(i) * (side + self.u(2.0));
        [x, self.margin(inset.top), side, side]
    }
    /// How far the row of key caps is moved: as one, so its caps stay together at the edge.
    fn caps_shift(&self) -> [f32; 2] {
        let count = CONTROLS
            .iter()
            .filter(|c| matches!(c.place, Place::Cap(_)))
            .count();
        let Some(last) = count.checked_sub(1) else {
            return [0.0, 0.0];
        };
        let [x, y, ..] = self.cap_at_rest(0);
        let [lx, _, side, _] = self.cap_at_rest(last as u8);
        self.shift(Control::Caps, [x, y, lx + side - x, side])
    }
    /// The Fn panel, in the middle of the screen on whole GUI pixels.
    pub(super) fn panel(&self) -> Rect {
        let span = |n: u8, tile: f32| {
            let n = f32::from(n);
            n * tile + (n - 1.0) * TILE_GAP + 2.0 * PANEL_PADDING
        };
        let (w, h) = (span(PANEL_COLUMNS, TILE[0]), span(PANEL_ROWS, TILE[1]));
        let x = ((self.size[0] / self.gui - w) / 2.0).floor();
        let y = ((self.size[1] / self.gui - h) / 2.0).floor();
        [x * self.gui, y * self.gui, w * self.gui, h * self.gui]
    }
    /// The whole screen.
    pub(super) fn screen(&self) -> Rect {
        [0.0, 0.0, self.size[0], self.size[1]]
    }
    /// The joystick's side of the screen, and where it rests: the D-pad's middle.
    pub(super) fn stick_zone(&self, at: [f32; 2]) -> bool {
        at[0] < self.size[0] * STICK_SHARE
    }
    pub(super) fn stick_home(&self) -> [f32; 2] {
        let [x, y, side, _] = self.dpad();
        [x + side / 2.0, y + side / 2.0]
    }
    pub(super) fn reach(&self) -> f32 {
        self.u(REACH)
    }
    /// How far a finger may drift and still tap or hold.
    pub(super) fn slop(&self) -> f32 {
        self.u(4.0)
    }
    /// How far above a finger things stay in sight: a fingertip, about 33 points.
    pub(super) fn finger_clearance(&self) -> f32 {
        self.u(FINGER)
    }
    /// A small button's square over a finger at `at`, clear of the fingertip, kept on screen.
    pub(super) fn above(&self, at: [f32; 2]) -> Rect {
        let side = self.u(SMALL);
        let x = (at[0] - side / 2.0).clamp(0.0, self.size[0] - side);
        let y = (at[1] - self.finger_clearance() - side).max(0.0);
        [x, y, side, side]
    }
}

pub(super) fn contains(r: Rect, at: [f32; 2]) -> bool {
    (r[0]..r[0] + r[2]).contains(&at[0]) && (r[1]..r[1] + r[3]).contains(&at[1])
}

#[cfg(test)]
mod tests {
    use super::super::functions::CONTROLS;
    use super::*;

    /// Phones and tablets in landscape, with their pixels per point: an iPhone SE, an iPhone,
    /// an iPhone Pro Max, the iPhone Duo unfolded, an iPad mini and a 13-inch iPad Pro.
    const SCREENS: [(u32, u32, f64); 6] = [
        (1334, 750, 2.0),
        (2622, 1206, 3.0),
        (2868, 1320, 3.0),
        (2853, 2007, 3.0),
        (2266, 1488, 2.0),
        (2752, 2064, 2.0),
    ];

    fn overlap(a: Rect, b: Rect) -> bool {
        a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
    }

    #[test]
    fn the_controls_fit_every_screen_without_overlapping() {
        for (w, h, scale) in SCREENS {
            let layout = Layout::new(w, h, scale);
            let mut rects = vec![
                ("D-pad", layout.dpad()),
                ("hotbar", layout.hotbar()),
                ("jump", layout.button(Button::Jump)),
                ("inventory", layout.button(Button::Inventory)),
                ("pause", layout.button(Button::Pause)),
                ("chat", layout.button(Button::Chat)),
            ];
            rects.extend(
                CONTROLS
                    .iter()
                    .filter(|c| matches!(c.place, Place::Cap(_)))
                    .map(|c| (c.keys, layout.control(c.place))),
            );
            let slack = layout.gui;
            for (name, r) in &rects {
                assert!(
                    r[0] >= 0.0
                        && r[1] >= 0.0
                        && r[0] + r[2] <= w as f32
                        && r[1] + r[3] <= h as f32 + slack,
                    "{w}x{h}: the {name} leaves the screen"
                );
            }
            while let Some((a, ra)) = rects.pop() {
                for (b, rb) in &rects {
                    assert!(!overlap(ra, *rb), "{w}x{h}: the {a} and the {b} overlap");
                }
            }
        }
    }

    #[test]
    fn the_fn_panel_fits_every_screen_clear_of_back_with_its_tiles_apart() {
        for (w, h, scale) in SCREENS {
            let layout = Layout::new(w, h, scale);
            let panel = layout.panel();
            let inside = |r: Rect, of: Rect| {
                r[0] >= of[0]
                    && r[1] >= of[1]
                    && r[0] + r[2] <= of[0] + of[2]
                    && r[1] + r[3] <= of[1] + of[3]
            };
            assert!(
                inside(panel, layout.screen()),
                "{w}x{h}: the panel leaves the screen"
            );
            assert!(
                !overlap(panel, layout.button(Button::Back)),
                "{w}x{h}: the panel on Back"
            );
            let mut tiles: Vec<_> = CONTROLS
                .iter()
                .filter(|c| matches!(c.place, Place::Tile(_)))
                .map(|c| (c.keys, layout.control(c.place)))
                .collect();
            assert_eq!(tiles.len(), 6);
            while let Some((a, ra)) = tiles.pop() {
                assert!(inside(ra, panel), "{w}x{h}: {a} leaves the panel");
                for (b, rb) in &tiles {
                    assert!(!overlap(ra, *rb), "{w}x{h}: {a} and {b} overlap");
                }
            }
        }
    }

    #[test]
    fn shift_stays_clear_of_back_and_of_an_inventory_with_its_recipe_book() {
        for (w, h, scale) in SCREENS {
            let layout = Layout::new(w, h, scale);
            let shift = layout.button(Button::Shift);
            let g = layout.gui;
            // The crafting book (147 GUI pixels) beside the inventory (176), which it pushes right.
            let (gw, gh) = (w as f32 / g, h as f32 / g);
            let panel = [
                (gw / 2.0 - 88.0 - 77.0) * g,
                (gh / 2.0 - 83.0) * g,
                323.0 * g,
                166.0 * g,
            ];
            assert!(
                shift[0] >= 0.0 && shift[1] + shift[3] <= h as f32,
                "{w}x{h}: Shift leaves the screen"
            );
            assert!(
                !overlap(shift, layout.button(Button::Back)),
                "{w}x{h}: Shift on Back"
            );
            assert!(!overlap(shift, panel), "{w}x{h}: Shift on the inventory");
        }
    }

    #[test]
    fn a_finger_on_the_dpad_finds_its_square() {
        let layout = Layout::new(2622, 1206, 3.0);
        for cell in Cell::ALL {
            let [x, y, s, _] = layout.cell(cell);
            assert_eq!(layout.cell_at([x + s / 2.0, y + s / 2.0]), Some(cell));
        }
        let [x, y, side, _] = layout.dpad();
        assert_eq!(layout.cell_at([x + 1.0, y + side - 1.0]), None);
        assert_eq!(layout.cell_at([x + side + 1.0, y]), None);
    }

    use super::super::config::{Insets, Tweak};

    /// The controls that move, as rectangles.
    fn movable(layout: &Layout) -> Vec<(&'static str, Rect)> {
        let mut rects = vec![
            ("D-pad", layout.dpad()),
            ("jump", layout.button(Button::Jump)),
            ("pause", layout.button(Button::Pause)),
            ("back", layout.button(Button::Back)),
            ("shift", layout.button(Button::Shift)),
            ("chat", layout.button(Button::Chat)),
        ];
        rects.extend(
            CONTROLS
                .iter()
                .filter(|c| matches!(c.place, Place::Cap(_)))
                .map(|c| (c.keys, layout.control(c.place))),
        );
        rects
    }

    fn config_with(control: Control, tweak: Tweak) -> TouchConfig {
        let mut config = TouchConfig::default();
        config.set(control, tweak);
        config
    }

    #[test]
    fn the_default_configuration_is_pocket_editions_layout() {
        for (w, h, scale) in SCREENS {
            let plain = Layout::new(w, h, scale);
            let configured = Layout::with_config(w, h, scale, TouchConfig::default());
            assert_eq!(plain.placement(), configured.placement());
            for ((name, a), (_, b)) in movable(&plain).into_iter().zip(movable(&configured)) {
                assert_eq!(a, b, "{w}x{h}: the {name}");
            }
        }
    }

    #[test]
    fn a_control_moves_by_its_offset_in_screen_shares() {
        for (w, h, scale) in SCREENS {
            let rest = Layout::new(w, h, scale).button(Button::Jump);
            let config = config_with(
                Control::Jump,
                Tweak {
                    dx: -0.1,
                    dy: -0.05,
                    ..Tweak::default()
                },
            );
            let moved = Layout::with_config(w, h, scale, config).button(Button::Jump);
            assert!(
                (moved[0] - (rest[0] - 0.1 * w as f32)).abs() < 0.01,
                "{w}x{h}"
            );
            assert!(
                (moved[1] - (rest[1] - 0.05 * h as f32)).abs() < 0.01,
                "{w}x{h}"
            );
            assert_eq!(
                (moved[2], moved[3]),
                (rest[2], rest[3]),
                "its size is its own"
            );
        }
    }

    #[test]
    fn a_moved_control_cannot_leave_the_screen() {
        for (w, h, scale) in SCREENS {
            for control in Control::ALL {
                for (dx, dy) in [(1.0, 1.0), (-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0)] {
                    let config = config_with(
                        control,
                        Tweak {
                            dx,
                            dy,
                            ..Tweak::default()
                        },
                    );
                    for (name, r) in movable(&Layout::with_config(w, h, scale, config)) {
                        assert!(
                            r[0] >= 0.0
                                && r[1] >= 0.0
                                && r[0] + r[2] <= w as f32
                                && r[1] + r[3] <= h as f32,
                            "{w}x{h}: the {name} leaves the screen with the {control:?} at ({dx}, {dy})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_dpad_takes_its_squares_and_the_joystick_along() {
        let (w, h, scale) = (2622, 1206, 3.0);
        let rest = Layout::new(w, h, scale);
        let config = config_with(
            Control::DPad,
            Tweak {
                dx: 0.3,
                dy: -0.1,
                ..Tweak::default()
            },
        );
        let moved = Layout::with_config(w, h, scale, config);
        let shift = [
            moved.dpad()[0] - rest.dpad()[0],
            moved.dpad()[1] - rest.dpad()[1],
        ];
        assert!(shift[0] > 0.0 && shift[1] < 0.0);
        for cell in Cell::ALL {
            let (a, b) = (rest.cell(cell), moved.cell(cell));
            let near = |x: f32, y: f32| (x - y).abs() < 0.01;
            assert!(
                near(b[0] - a[0], shift[0]) && near(b[1] - a[1], shift[1]),
                "{cell:?}"
            );
            let middle = [b[0] + b[2] / 2.0, b[1] + b[3] / 2.0];
            assert_eq!(moved.cell_at(middle), Some(cell), "a finger on it finds it");
        }
        let (a, b) = (rest.stick_home(), moved.stick_home());
        assert!((b[0] - a[0] - shift[0]).abs() < 0.01 && (b[1] - a[1] - shift[1]).abs() < 0.01);
    }

    #[test]
    fn the_key_caps_move_as_a_row_and_stay_together_at_the_edge() {
        let (w, h, scale) = (2622, 1206, 3.0);
        let caps = |layout: &Layout| -> Vec<Rect> {
            CONTROLS
                .iter()
                .filter(|c| matches!(c.place, Place::Cap(_)))
                .map(|c| layout.control(c.place))
                .collect()
        };
        let rest = caps(&Layout::new(w, h, scale));
        for dx in [0.2, 1.0] {
            let config = config_with(
                Control::Caps,
                Tweak {
                    dx,
                    ..Tweak::default()
                },
            );
            let moved = caps(&Layout::with_config(w, h, scale, config));
            let shift = moved[0][0] - rest[0][0];
            assert!(shift > 0.0);
            for (a, b) in rest.iter().zip(&moved) {
                assert_eq!(b[0] - a[0], shift, "one shift for the whole row");
                assert_eq!(b[1], a[1]);
            }
            let last = moved.last().unwrap();
            assert!(last[0] + last[2] <= w as f32, "the row fits the screen");
        }
    }

    #[test]
    fn chat_follows_pause_and_the_inventory_button_stays_with_the_hotbar() {
        let (w, h, scale) = (2622, 1206, 3.0);
        let rest = Layout::new(w, h, scale);
        let config = config_with(
            Control::Pause,
            Tweak {
                dx: -0.2,
                ..Tweak::default()
            },
        );
        let moved = Layout::with_config(w, h, scale, config);
        let shift = moved.button(Button::Pause)[0] - rest.button(Button::Pause)[0];
        assert!(shift < 0.0);
        assert_eq!(
            moved.button(Button::Chat)[0] - rest.button(Button::Chat)[0],
            shift
        );
        let config = config_with(
            Control::DPad,
            Tweak {
                dx: 0.4,
                dy: -0.3,
                ..Tweak::default()
            },
        );
        let moved = Layout::with_config(w, h, scale, config);
        assert_eq!(
            moved.button(Button::Inventory),
            rest.button(Button::Inventory)
        );
        assert_eq!(moved.hotbar(), rest.hotbar());
    }

    /// A landscape phone's insets: the notch's side, the corners and the home indicator.
    fn notched(scale: f64) -> Insets {
        let s = scale as f32;
        Insets {
            top: 0.0,
            left: 59.0 * s,
            bottom: 21.0 * s,
            right: 59.0 * s,
        }
    }

    #[test]
    fn the_controls_keep_clear_of_the_notch_and_the_home_indicator() {
        for (w, h, scale) in SCREENS {
            let mut config = TouchConfig::default();
            config.insets = notched(scale);
            let i = config.insets;
            let layout = Layout::with_config(w, h, scale, config);
            for (name, r) in movable(&layout) {
                // Shift is the keyboard's key: it takes the left-hand corner with the D-pad.
                assert!(
                    r[0] >= i.left
                        && r[1] >= i.top
                        && r[0] + r[2] <= w as f32 - i.right
                        && r[1] + r[3] <= h as f32 - i.bottom,
                    "{w}x{h}: the {name} is under the notch or the home indicator: {r:?}"
                );
            }
            let jump = layout.button(Button::Jump);
            let dpad = layout.dpad();
            let hotbar = layout.hotbar();
            assert!(
                !overlap(dpad, hotbar),
                "{w}x{h}: the D-pad and the hotbar overlap"
            );
            assert!(
                !overlap(jump, hotbar),
                "{w}x{h}: jump and the hotbar overlap"
            );
        }
    }

    #[test]
    fn a_moved_control_stays_inside_the_safe_area() {
        for (w, h, scale) in SCREENS {
            let mut config = config_with(
                Control::Back,
                Tweak {
                    dx: -1.0,
                    dy: -1.0,
                    ..Tweak::default()
                },
            );
            config.insets = notched(scale);
            let i = config.insets;
            let back = Layout::with_config(w, h, scale, config).button(Button::Back);
            assert!(back[0] >= i.left && back[1] >= i.top, "{w}x{h}: {back:?}");
        }
    }

    #[test]
    fn a_new_configuration_is_another_placement() {
        let (w, h, scale) = (2622, 1206, 3.0);
        let base = Layout::new(w, h, scale).placement();
        let config = config_with(
            Control::Jump,
            Tweak {
                dx: 0.01,
                ..Tweak::default()
            },
        );
        let moved = Layout::with_config(w, h, scale, config).placement();
        assert_ne!(base, moved, "a moved control ends the touches in progress");
        assert_eq!(moved, Layout::with_config(w, h, scale, config).placement());
        let mut notch = TouchConfig::default();
        notch.insets = notched(scale);
        assert_ne!(base, Layout::with_config(w, h, scale, notch).placement());
    }

    #[test]
    fn a_resize_or_another_display_is_another_placement() {
        let duo = Layout::new(2853, 2007, 3.0).placement();
        assert_eq!(duo, Layout::new(2853, 2007, 3.0).placement());
        // Narrower, at the same GUI scale.
        assert_ne!(duo, Layout::new(2622, 2007, 3.0).placement(), "resized");
        assert_ne!(
            duo,
            Layout::new(2853, 2007, 2.0).placement(),
            "another display"
        );
    }

    #[test]
    fn buttons_are_a_thumb_wide_on_phones_and_tablets() {
        for (w, h, scale) in SCREENS {
            let layout = Layout::new(w, h, scale);
            let points = layout.cell(Cell::Forward)[2] / scale as f32;
            assert!((44.0..=60.0).contains(&points), "{w}x{h}: {points} pt");
        }
    }
}
