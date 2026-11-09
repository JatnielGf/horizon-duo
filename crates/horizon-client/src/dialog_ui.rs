//! Where the parts of a dialog sit on the screen, and what a click at a point hits. It works in
//! GUI pixels and measures text through a function it is given, so it draws nothing and the
//! game's font is not needed to test it.
use crate::dialog::{Control, Dialog};

pub type Rect = [f32; 4];

/// The widest the dialog's content is, in GUI pixels.
const CONTENT_WIDTH: f32 = 280.0;
/// Space kept free at the screen's edges.
const EDGE: f32 = 8.0;
const LINE: f32 = 11.0;
const ROW: f32 = 20.0;
const GAP: f32 = 6.0;
const BUTTON_GAP: f32 = 4.0;

/// What a click can land on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// Input `n`.
    Input(usize),
    /// Button `n`, counting the way out last (`Dialog::all_buttons`).
    Button(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlacedInput {
    /// The label above the control and where it goes (its y), when it is shown.
    pub label: Option<(String, f32)>,
    pub rect: Rect,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    /// The title and its y; it is centered across the screen.
    pub title: Option<(String, f32)>,
    /// Each line of the body, wrapped, and its y.
    pub lines: Vec<(String, f32)>,
    pub inputs: Vec<PlacedInput>,
    /// One for each of `Dialog::all_buttons`.
    pub buttons: Vec<Rect>,
    /// The body had to be cut short to leave room for the inputs and the buttons.
    pub cut: bool,
    pub center_x: f32,
}

/// `text` split into lines no wider than `limit`, at spaces (a word wider than it is cut).
fn wrap(text: &str, limit: f32, width: &dyn Fn(&str) -> f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split(' ') {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if width(&candidate) <= limit || line.is_empty() && word.is_empty() {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            // A word alone may still be too wide.
            let mut rest = word;
            while width(rest) > limit && rest.chars().count() > 1 {
                let mut cut = rest.chars().count() - 1;
                while cut > 1 && width(&rest.chars().take(cut).collect::<String>()) > limit {
                    cut -= 1;
                }
                let at = rest.char_indices().nth(cut).map_or(rest.len(), |(i, _)| i);
                lines.push(rest[..at].to_owned());
                rest = &rest[at..];
            }
            line = rest.to_owned();
        }
        lines.push(line);
    }
    lines
}

/// The rows of buttons: `columns` to a row, each as wide as it says (and no wider than its share
/// of the content), the last row centered.
fn button_rows(widths: &[f32], columns: usize, content: f32) -> Vec<Vec<f32>> {
    let share = (content - BUTTON_GAP * (columns as f32 - 1.0)) / columns as f32;
    widths
        .chunks(columns.max(1))
        .map(|row| row.iter().map(|w| w.min(share.max(40.0))).collect())
        .collect()
}

pub fn place(dialog: &Dialog, size: (f32, f32), width: &dyn Fn(&str) -> f32) -> Placed {
    let (screen_w, screen_h) = size;
    let content = CONTENT_WIDTH.min(screen_w - 2.0 * EDGE).max(60.0);
    let center_x = (screen_w / 2.0).floor();

    // The fixed parts first: they must fit, the body gives way.
    let mut fixed = 0.0;
    let has_title = !dialog.title.is_empty();
    if has_title {
        fixed += LINE + GAP;
    }
    for input in &dialog.inputs {
        let label = input.label_visible && !input.label.is_empty();
        let shown_as_label = matches!(input.control, Control::Text { .. });
        fixed += if label && shown_as_label { LINE } else { 0.0 } + ROW + GAP;
    }
    let buttons: Vec<f32> = dialog.all_buttons().map(|b| b.width).collect();
    let in_grid = dialog.buttons.len();
    let grid_columns = dialog.columns.clamp(1, 8).min(in_grid.max(1));
    let grid = button_rows(&buttons[..in_grid], grid_columns, content);
    let exit_row = usize::from(dialog.exit.is_some());
    fixed += (grid.len() + exit_row) as f32 * (ROW + BUTTON_GAP);

    let body: Vec<String> = dialog
        .body
        .iter()
        .flat_map(|text| wrap(text, content, width))
        .collect();
    let room = (screen_h - 2.0 * EDGE - fixed - GAP).max(0.0);
    let fits = (room / LINE).floor() as usize;
    let cut = body.len() > fits;
    let shown: Vec<String> = body.into_iter().take(fits).collect();
    let body_height = if shown.is_empty() {
        0.0
    } else {
        shown.len() as f32 * LINE + GAP
    };

    // The whole, centered across the screen's height.
    let total = fixed + body_height;
    let mut y = ((screen_h - total) / 2.0).floor().max(EDGE);

    let title = has_title.then(|| {
        let at = y;
        y += LINE + GAP;
        (dialog.title.clone(), at)
    });
    let lines = shown
        .into_iter()
        .map(|line| {
            let at = y;
            y += LINE;
            (line, at)
        })
        .collect::<Vec<_>>();
    if !lines.is_empty() {
        y += GAP;
    }

    let mut inputs = Vec::new();
    for input in &dialog.inputs {
        let shown_as_label = matches!(input.control, Control::Text { .. });
        let label = (input.label_visible && !input.label.is_empty() && shown_as_label).then(|| {
            let at = y;
            y += LINE;
            (input.label.clone(), at)
        });
        let w = input.width.min(content);
        inputs.push(PlacedInput {
            label,
            rect: [center_x - (w / 2.0).floor(), y, w, ROW],
        });
        y += ROW + GAP;
    }

    let mut placed_buttons = Vec::new();
    for row in &grid {
        let span: f32 = row.iter().sum::<f32>() + BUTTON_GAP * (row.len() as f32 - 1.0);
        let mut x = center_x - (span / 2.0).floor();
        for w in row {
            placed_buttons.push([x, y, *w, ROW]);
            x += w + BUTTON_GAP;
        }
        y += ROW + BUTTON_GAP;
    }
    if let Some(exit) = &dialog.exit {
        let w = exit.width.min(content);
        placed_buttons.push([center_x - (w / 2.0).floor(), y, w, ROW]);
    }

    Placed {
        title,
        lines,
        inputs,
        buttons: placed_buttons,
        cut,
        center_x,
    }
}

fn inside(r: &Rect, x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

impl Placed {
    /// What a click at (`x`, `y`) lands on.
    pub fn hit(&self, x: f32, y: f32) -> Option<Hit> {
        self.buttons
            .iter()
            .position(|r| inside(r, x, y))
            .map(Hit::Button)
            .or_else(|| {
                self.inputs
                    .iter()
                    .position(|i| inside(&i.rect, x, y))
                    .map(Hit::Input)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_protocol::text::Language;
    use horizon_world::nbt::Tag;

    /// Six pixels a character, as the game's font is near enough.
    fn six(text: &str) -> f32 {
        text.chars().count() as f32 * 6.0
    }

    fn s(v: &str) -> Tag {
        Tag::String(v.into())
    }

    fn login() -> Dialog {
        let tag = Tag::compound([
            ("type", s("multi_action")),
            ("title", s("Login")),
            (
                "body",
                Tag::compound([
                    ("type", s("plain_message")),
                    (
                        "contents",
                        s("Welcome back to the server, Jatniel. Please log in to play with us."),
                    ),
                ]),
            ),
            (
                "inputs",
                Tag::List(vec![Tag::compound([
                    ("type", s("text")),
                    ("key", s("password")),
                    ("label", s("Password")),
                ])]),
            ),
            (
                "actions",
                Tag::List(vec![Tag::compound([
                    ("label", s("Login")),
                    (
                        "action",
                        Tag::compound([("type", s("dynamic/custom")), ("id", s("a:b"))]),
                    ),
                ])]),
            ),
            ("exit_action", Tag::compound([("label", s("Leave"))])),
        ]);
        Dialog::read(&tag, &Language::new())
    }

    fn inside_screen(r: &Rect, size: (f32, f32)) -> bool {
        r[0] >= 0.0 && r[1] >= 0.0 && r[0] + r[2] <= size.0 && r[1] + r[3] <= size.1
    }

    #[test]
    fn words_wrap_at_spaces_and_long_words_are_cut() {
        let lines = wrap("one two three four", 55.0, &six);
        assert_eq!(lines, ["one two", "three", "four"]);
        let lines = wrap("abcdefghijklmnop", 30.0, &six);
        assert!(
            lines.len() >= 3 && lines.iter().all(|l| six(l) <= 30.0),
            "{lines:?}"
        );
        assert_eq!(wrap("a\nb", 60.0, &six), ["a", "b"]);
        assert_eq!(wrap("", 60.0, &six), [""]);
    }

    #[test]
    fn a_login_dialog_fits_every_screen_and_its_parts_do_not_overlap() {
        for size in [
            (480.0, 262.0),
            (320.0, 180.0),
            (427.0, 240.0),
            (874.0, 402.0),
        ] {
            let d = login();
            let placed = place(&d, size, &six);
            assert_eq!(placed.buttons.len(), 2, "the Login button and the way out");
            assert_eq!(placed.inputs.len(), 1);
            let mut rects: Vec<Rect> = placed.buttons.clone();
            rects.push(placed.inputs[0].rect);
            for r in &rects {
                assert!(inside_screen(r, size), "{size:?}: {r:?}");
            }
            for (i, a) in rects.iter().enumerate() {
                for b in rects.iter().skip(i + 1) {
                    let overlap = a[0] < b[0] + b[2]
                        && b[0] < a[0] + a[2]
                        && a[1] < b[1] + b[3]
                        && b[1] < a[1] + a[3];
                    assert!(!overlap, "{size:?}: {a:?} and {b:?}");
                }
            }
            // The text sits above the controls.
            let first = placed.inputs[0].rect[1];
            let text_ys = placed
                .lines
                .iter()
                .map(|(_, y)| *y)
                .chain(placed.title.iter().map(|(_, y)| *y));
            for y in text_ys {
                assert!(y < first, "{size:?}");
            }
        }
    }

    #[test]
    fn a_small_screen_cuts_the_body_and_keeps_the_controls() {
        let mut d = login();
        d.body = vec!["word ".repeat(200)];
        let size = (320.0, 120.0);
        let placed = place(&d, size, &six);
        assert!(placed.cut, "the body gave way");
        assert_eq!(placed.buttons.len(), 2);
        for r in placed
            .buttons
            .iter()
            .chain(placed.inputs.iter().map(|i| &i.rect))
        {
            assert!(inside_screen(r, size), "{r:?}");
        }
    }

    #[test]
    fn a_click_finds_the_button_or_input_under_it() {
        let d = login();
        let placed = place(&d, (480.0, 262.0), &six);
        let login_button = placed.buttons[0];
        let leave = placed.buttons[1];
        let input = placed.inputs[0].rect;
        let middle = |r: Rect| (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        let (x, y) = middle(login_button);
        assert_eq!(placed.hit(x, y), Some(Hit::Button(0)));
        let (x, y) = middle(leave);
        assert_eq!(placed.hit(x, y), Some(Hit::Button(1)));
        let (x, y) = middle(input);
        assert_eq!(placed.hit(x, y), Some(Hit::Input(0)));
        assert_eq!(placed.hit(1.0, 1.0), None);
    }

    #[test]
    fn buttons_are_laid_out_in_the_columns_the_dialog_asks_for() {
        let actions: Vec<Tag> = (0..5)
            .map(|n| {
                Tag::compound([
                    ("label", s(&format!("Option {n}"))),
                    ("width", Tag::Int(80)),
                ])
            })
            .collect();
        let tag = Tag::compound([
            ("type", s("multi_action")),
            ("title", s("Menu")),
            ("columns", Tag::Int(2)),
            ("actions", Tag::List(actions)),
        ]);
        let d = Dialog::read(&tag, &Language::new());
        let placed = place(&d, (480.0, 262.0), &six);
        assert_eq!(placed.buttons.len(), 5);
        let rows: std::collections::BTreeSet<i32> =
            placed.buttons.iter().map(|r| r[1] as i32).collect();
        assert_eq!(rows.len(), 3, "two to a row: three rows");
        // The first two share a row.
        assert_eq!(placed.buttons[0][1], placed.buttons[1][1]);
        assert!(placed.buttons[0][0] < placed.buttons[1][0]);
    }

    #[test]
    fn a_dialog_with_nothing_but_a_title_still_has_its_way_out() {
        let tag = Tag::compound([("type", s("notice")), ("title", s("Hi"))]);
        let d = Dialog::read(&tag, &Language::new());
        let placed = place(&d, (200.0, 100.0), &six);
        assert_eq!(placed.buttons.len(), 1);
        assert!(inside_screen(&placed.buttons[0], (200.0, 100.0)));
    }
}
