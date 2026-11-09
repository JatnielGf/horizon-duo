//! Server dialogs: a screen a server shows (title, text, inputs and buttons) and whose answer it
//! waits for. AuthMe 6 logs players in with one, before they join and after. This reads one from
//! the NBT the server sent, keeps what the player typed, and says what a button press asks for;
//! it draws nothing and sends nothing.
//!
//! The dialog types vanilla has are all read (`notice`, `confirmation`, `multi_action`,
//! `server_links`, `dialog_list`); what a dialog says is flattened to plain text, so its colors
//! are not shown yet. An action the client can't carry out is read and then does nothing.
use horizon_protocol::text::{Language, plain};
use horizon_world::nbt::{JavaString, Tag};

/// How wide a button is when the dialog doesn't say, in GUI pixels.
const BUTTON_WIDTH: f32 = 150.0;
/// How long a text input may be when the dialog doesn't say.
const TEXT_LENGTH: usize = 32;

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Notice,
    Confirmation,
    MultiAction,
    ServerLinks,
    DialogList,
    /// A type this client doesn't know: shown with its text and an OK button.
    Other(String),
}

/// What happens to the dialog once a button is pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum After {
    Close,
    /// It stays, as it was.
    Nothing,
    /// It stays, and waits for the server to answer.
    Wait,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Does nothing (an OK button).
    None,
    /// ServerboundCustomClickAction: `id`, with the inputs' values when `dynamic`, else `payload`.
    Custom {
        id: String,
        dynamic: bool,
        payload: Option<Tag>,
    },
    /// Another dialog, inline.
    Show(Tag),
    RunCommand(String),
    SuggestCommand(String),
    OpenUrl(String),
    CopyToClipboard(String),
    /// An action this client doesn't carry out (a page of a dialog list, a macro).
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    pub label: String,
    pub tooltip: Option<String>,
    pub width: f32,
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Control {
    Text {
        initial: String,
        max_length: usize,
        /// Whether it takes more than one line.
        multiline: bool,
    },
    Boolean {
        initial: bool,
        on_true: String,
        on_false: String,
    },
    /// The options' ids and shown names, and the one that starts selected.
    Choice {
        options: Vec<(String, String)>,
        initial: usize,
    },
    Range {
        start: f32,
        end: f32,
        initial: f32,
        /// The step; none means any value.
        step: Option<f32>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    /// The name its value goes under in the answer.
    pub key: String,
    pub label: String,
    pub label_visible: bool,
    pub width: f32,
    pub control: Control,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Dialog {
    pub kind: Kind,
    pub title: String,
    pub body: Vec<String>,
    pub inputs: Vec<Input>,
    pub buttons: Vec<Button>,
    /// The button that leaves a dialog (multi-action, server links, dialog list).
    pub exit: Option<Button>,
    pub escape_closes: bool,
    pub after: After,
    pub columns: usize,
}

fn get<'t>(tag: &'t Tag, key: &str) -> Option<&'t Tag> {
    match tag {
        Tag::Compound(map) => map.get(&JavaString::from(key)),
        _ => None,
    }
}

fn string(tag: &Tag, key: &str) -> Option<String> {
    match get(tag, key)? {
        Tag::String(s) => Some(s.to_string_lossy()),
        _ => None,
    }
}

fn integer(tag: &Tag) -> Option<i64> {
    match tag {
        Tag::Byte(v) => Some(i64::from(*v)),
        Tag::Short(v) => Some(i64::from(*v)),
        Tag::Int(v) => Some(i64::from(*v)),
        Tag::Long(v) => Some(*v),
        _ => None,
    }
}

fn number(tag: &Tag) -> Option<f32> {
    match tag {
        Tag::Float(bits) => Some(f32::from_bits(*bits)),
        Tag::Double(bits) => Some(f64::from_bits(*bits) as f32),
        other => integer(other).map(|v| v as f32),
    }
}

fn boolean(tag: &Tag, key: &str, default: bool) -> bool {
    get(tag, key).and_then(integer).map_or(default, |v| v != 0)
}

/// The elements of a list, or the one element a single value stands for.
fn many(tag: &Tag) -> Vec<&Tag> {
    match tag {
        Tag::List(items) => items.iter().collect(),
        other => vec![other],
    }
}

/// `minecraft:notice` and `notice` are the same type.
fn kind_of(tag: &Tag) -> String {
    let name = string(tag, "type").unwrap_or_default();
    name.strip_prefix("minecraft:").unwrap_or(&name).to_owned()
}

fn text(tag: &Tag, key: &str, lang: &Language) -> Option<String> {
    get(tag, key).map(|component| plain(component, lang))
}

fn read_action(tag: &Tag) -> Action {
    let kind = kind_of(tag);
    let own = |key: &str| string(tag, key).unwrap_or_default();
    match kind.as_str() {
        "custom" => Action::Custom {
            id: own("id"),
            dynamic: false,
            payload: get(tag, "payload").cloned(),
        },
        "dynamic/custom" => Action::Custom {
            id: own("id"),
            dynamic: true,
            payload: get(tag, "additions").cloned(),
        },
        "show_dialog" => match get(tag, "dialog") {
            Some(dialog @ Tag::Compound(_)) => Action::Show(dialog.clone()),
            _ => Action::Unsupported(kind),
        },
        "run_command" => Action::RunCommand(own("command")),
        "suggest_command" => Action::SuggestCommand(own("command")),
        "open_url" => Action::OpenUrl(own("url")),
        "copy_to_clipboard" => Action::CopyToClipboard(own("value")),
        _ => Action::Unsupported(kind),
    }
}

fn read_button(tag: &Tag, lang: &Language) -> Button {
    Button {
        label: text(tag, "label", lang).unwrap_or_default(),
        tooltip: text(tag, "tooltip", lang),
        width: get(tag, "width")
            .and_then(number)
            .filter(|w| *w >= 1.0)
            .unwrap_or(BUTTON_WIDTH),
        action: get(tag, "action").map_or(Action::None, read_action),
    }
}

fn read_input(tag: &Tag, lang: &Language) -> Option<Input> {
    let key = string(tag, "key")?;
    let label = text(tag, "label", lang).unwrap_or_default();
    let control = match kind_of(tag).as_str() {
        "text" => Control::Text {
            initial: string(tag, "initial").unwrap_or_default(),
            max_length: get(tag, "max_length")
                .and_then(integer)
                .map_or(TEXT_LENGTH, |n| n.clamp(1, 32767) as usize),
            multiline: get(tag, "multiline").is_some(),
        },
        "boolean" => Control::Boolean {
            initial: boolean(tag, "initial", false),
            on_true: string(tag, "on_true").unwrap_or_else(|| "true".into()),
            on_false: string(tag, "on_false").unwrap_or_else(|| "false".into()),
        },
        "single_option" => {
            let options: Vec<(String, String)> = get(tag, "options")
                .map(many)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|option| match option {
                    // An option is its id alone, or an id with a name to show.
                    Tag::String(id) => Some((id.to_string_lossy(), id.to_string_lossy())),
                    Tag::Compound(_) => {
                        let id = string(option, "id")?;
                        let shown = text(option, "display", lang).unwrap_or_else(|| id.clone());
                        Some((id, shown))
                    }
                    _ => None,
                })
                .collect();
            let initial = get(tag, "options")
                .map(many)
                .unwrap_or_default()
                .into_iter()
                .filter(|o| matches!(o, Tag::String(_) | Tag::Compound(_)))
                .position(|o| boolean(o, "initial", false))
                .unwrap_or(0);
            if options.is_empty() {
                return None;
            }
            Control::Choice { options, initial }
        }
        "number_range" => {
            let start = get(tag, "start").and_then(number).unwrap_or(0.0);
            let end = get(tag, "end").and_then(number).unwrap_or(start);
            let (low, high) = (start.min(end), start.max(end));
            Control::Range {
                start: low,
                end: high,
                initial: get(tag, "initial")
                    .and_then(number)
                    .unwrap_or(low)
                    .clamp(low, high),
                step: get(tag, "step").and_then(number).filter(|s| *s > 0.0),
            }
        }
        _ => return None,
    };
    Some(Input {
        key,
        label,
        label_visible: boolean(tag, "label_visible", true),
        width: get(tag, "width")
            .and_then(number)
            .filter(|w| *w >= 1.0)
            .unwrap_or(200.0),
        control,
    })
}

/// A dialog's body: a message, or an item with a description (only the description is shown).
fn read_body(tag: &Tag, lang: &Language) -> Vec<String> {
    let Some(body) = get(tag, "body") else {
        return Vec::new();
    };
    many(body)
        .into_iter()
        .filter_map(|part| match kind_of(part).as_str() {
            "plain_message" => text(part, "contents", lang),
            "item" => get(part, "description").and_then(|d| text(d, "contents", lang)),
            _ => None,
        })
        .collect()
}

impl Dialog {
    /// The dialog in `tag`, with its texts in `lang`.
    pub fn read(tag: &Tag, lang: &Language) -> Dialog {
        let kind = match kind_of(tag).as_str() {
            "notice" => Kind::Notice,
            "confirmation" => Kind::Confirmation,
            "multi_action" => Kind::MultiAction,
            "server_links" => Kind::ServerLinks,
            "dialog_list" => Kind::DialogList,
            other => Kind::Other(other.to_owned()),
        };
        let ok = || Button {
            label: lang.get("gui.ok").cloned().unwrap_or_else(|| "OK".into()),
            tooltip: None,
            width: BUTTON_WIDTH,
            action: Action::None,
        };
        let button = |key: &str| get(tag, key).map(|b| read_button(b, lang));
        let mut exit = button("exit_action");
        let buttons = match &kind {
            Kind::Notice => vec![button("action").unwrap_or_else(ok)],
            Kind::Confirmation => button("yes").into_iter().chain(button("no")).collect(),
            Kind::MultiAction => get(tag, "actions")
                .map(many)
                .unwrap_or_default()
                .into_iter()
                .map(|b| read_button(b, lang))
                .collect(),
            Kind::DialogList => get(tag, "dialogs")
                .map(many)
                .unwrap_or_default()
                .into_iter()
                .filter(|d| matches!(d, Tag::Compound(_)))
                .map(|d| Button {
                    label: text(d, "title", lang).unwrap_or_default(),
                    tooltip: None,
                    width: get(tag, "button_width")
                        .and_then(number)
                        .filter(|w| *w >= 1.0)
                        .unwrap_or(BUTTON_WIDTH),
                    action: Action::Show(d.clone()),
                })
                .collect(),
            Kind::ServerLinks => Vec::new(),
            Kind::Other(_) => vec![ok()],
        };
        // A dialog nobody can leave would trap the player: it always has a way out.
        if buttons.is_empty() && exit.is_none() {
            exit = Some(ok());
        }
        Dialog {
            kind,
            title: text(tag, "title", lang).unwrap_or_default(),
            body: read_body(tag, lang),
            inputs: get(tag, "inputs")
                .map(many)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|input| read_input(input, lang))
                .collect(),
            buttons,
            exit,
            escape_closes: boolean(tag, "can_close_with_escape", true),
            after: match string(tag, "after_action").as_deref() {
                Some("none") => After::Nothing,
                Some("wait_for_response") => After::Wait,
                _ => After::Close,
            },
            columns: get(tag, "columns")
                .and_then(integer)
                .map_or(2, |n| n.clamp(1, 8) as usize),
        }
    }

    /// Every button, the way out last.
    pub fn all_buttons(&self) -> impl Iterator<Item = &Button> {
        self.buttons.iter().chain(self.exit.iter())
    }
}

/// What a control holds right now.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Text(String),
    Bool(bool),
    Choice(usize),
    Range(f32),
}

/// What the player has typed or picked in a dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct Form {
    values: Vec<Value>,
}

/// What a pressed button asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Nothing,
    /// Send ServerboundCustomClickAction.
    Send {
        id: String,
        payload: Option<Tag>,
    },
    /// Show this dialog (inline) instead.
    Show(Tag),
    RunCommand(String),
    SuggestCommand(String),
    OpenUrl(String),
    CopyToClipboard(String),
}

impl Form {
    pub fn new(dialog: &Dialog) -> Self {
        let values = dialog
            .inputs
            .iter()
            .map(|input| match &input.control {
                Control::Text {
                    initial,
                    max_length,
                    ..
                } => Value::Text(initial.chars().take(*max_length).collect()),
                Control::Boolean { initial, .. } => Value::Bool(*initial),
                Control::Choice { initial, .. } => Value::Choice(*initial),
                Control::Range { initial, .. } => Value::Range(*initial),
            })
            .collect();
        Self { values }
    }

    pub fn value(&self, input: usize) -> Option<&Value> {
        self.values.get(input)
    }

    /// Types into text input `input`, up to its length (a line break only where it takes lines).
    pub fn type_text(&mut self, dialog: &Dialog, input: usize, typed: &str) {
        let (
            Some(Value::Text(value)),
            Some(Control::Text {
                max_length,
                multiline,
                ..
            }),
        ) = (
            self.values.get_mut(input),
            dialog.inputs.get(input).map(|i| &i.control),
        )
        else {
            return;
        };
        for c in typed.chars() {
            let allowed = !c.is_control() || (*multiline && c == '\n');
            if allowed && value.chars().count() < *max_length {
                value.push(c);
            }
        }
    }

    pub fn backspace(&mut self, input: usize) {
        if let Some(Value::Text(value)) = self.values.get_mut(input) {
            value.pop();
        }
    }

    /// A tap on a boolean flips it, and on a choice moves to the next option.
    pub fn toggle(&mut self, dialog: &Dialog, input: usize) {
        match (
            self.values.get_mut(input),
            dialog.inputs.get(input).map(|i| &i.control),
        ) {
            (Some(Value::Bool(value)), Some(Control::Boolean { .. })) => *value = !*value,
            (Some(Value::Choice(at)), Some(Control::Choice { options, .. })) => {
                *at = (*at + 1) % options.len();
            }
            _ => {}
        }
    }

    /// Sets a number range from `share` of its track, from 0 to 1, on its steps.
    pub fn set_range(&mut self, dialog: &Dialog, input: usize, share: f32) {
        let (
            Some(Value::Range(value)),
            Some(Control::Range {
                start, end, step, ..
            }),
        ) = (
            self.values.get_mut(input),
            dialog.inputs.get(input).map(|i| &i.control),
        )
        else {
            return;
        };
        let mut at = start + (end - start) * share.clamp(0.0, 1.0);
        if let Some(step) = step {
            at = start + ((at - start) / step).round() * step;
        }
        *value = at.clamp(*start, *end);
    }

    /// What the dialog answers: each input's value under its key. A text and a choice are text,
    /// a boolean is a byte and a range is a float.
    pub fn answer(&self, dialog: &Dialog) -> Tag {
        let entries = dialog
            .inputs
            .iter()
            .zip(&self.values)
            .map(|(input, value)| {
                let tag = match (value, &input.control) {
                    (Value::Text(text), _) => Tag::String(text.as_str().into()),
                    (Value::Bool(on), _) => Tag::Byte(i8::from(*on)),
                    (Value::Choice(at), Control::Choice { options, .. }) => {
                        let id = options
                            .get(*at)
                            .map(|(id, _)| id.as_str())
                            .unwrap_or_default();
                        Tag::String(id.into())
                    }
                    (Value::Choice(_), _) => Tag::String("".into()),
                    (Value::Range(v), _) => Tag::Float(v.to_bits()),
                };
                (input.key.as_str(), tag)
            });
        Tag::compound(entries)
    }

    /// What pressing `button` asks for.
    pub fn press(&self, dialog: &Dialog, button: &Button) -> Outcome {
        match &button.action {
            Action::None | Action::Unsupported(_) => Outcome::Nothing,
            Action::Custom {
                id,
                dynamic,
                payload,
            } => {
                let payload = if *dynamic {
                    let mut answer = self.answer(dialog);
                    // The action's own additions go along with the inputs.
                    if let (Tag::Compound(values), Some(Tag::Compound(extra))) =
                        (&mut answer, payload)
                    {
                        for (key, value) in extra {
                            values.entry(key.clone()).or_insert_with(|| value.clone());
                        }
                    }
                    Some(answer)
                } else {
                    payload.clone()
                };
                Outcome::Send {
                    id: id.clone(),
                    payload,
                }
            }
            Action::Show(tag) => Outcome::Show(tag.clone()),
            Action::RunCommand(c) => Outcome::RunCommand(c.clone()),
            Action::SuggestCommand(c) => Outcome::SuggestCommand(c.clone()),
            Action::OpenUrl(u) => Outcome::OpenUrl(u.clone()),
            Action::CopyToClipboard(v) => Outcome::CopyToClipboard(v.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Tag {
        Tag::String(v.into())
    }
    fn c<const N: usize>(entries: [(&str, Tag); N]) -> Tag {
        Tag::compound(entries)
    }
    fn lang() -> Language {
        Language::new()
    }

    /// A login dialog the way AuthMe-style plugins send one: a text input and two buttons.
    fn login() -> Tag {
        c([
            ("type", s("minecraft:multi_action")),
            ("title", s("Login")),
            ("can_close_with_escape", Tag::Byte(0)),
            (
                "body",
                c([
                    ("type", s("minecraft:plain_message")),
                    ("contents", s("Welcome back, Jatniel!")),
                ]),
            ),
            (
                "inputs",
                Tag::List(vec![c([
                    ("type", s("minecraft:text")),
                    ("key", s("password")),
                    ("label", s("Password")),
                    ("max_length", Tag::Int(16)),
                ])]),
            ),
            (
                "actions",
                Tag::List(vec![c([
                    ("label", s("Login")),
                    ("width", Tag::Int(120)),
                    (
                        "action",
                        c([
                            ("type", s("minecraft:dynamic/custom")),
                            ("id", s("authme:login")),
                        ]),
                    ),
                ])]),
            ),
            (
                "exit_action",
                c([
                    ("label", s("Leave")),
                    (
                        "action",
                        c([("type", s("minecraft:custom")), ("id", s("authme:leave"))]),
                    ),
                ]),
            ),
        ])
    }

    #[test]
    fn a_login_dialog_is_read() {
        let d = Dialog::read(&login(), &lang());
        assert_eq!(d.kind, Kind::MultiAction);
        assert_eq!(d.title, "Login");
        assert_eq!(d.body, ["Welcome back, Jatniel!"]);
        assert!(!d.escape_closes, "Escape does not leave it");
        assert_eq!(d.after, After::Close);
        assert_eq!(d.inputs.len(), 1);
        assert_eq!(d.inputs[0].key, "password");
        assert_eq!(d.buttons.len(), 1);
        assert_eq!(d.buttons[0].width, 120.0);
        assert_eq!(d.exit.as_ref().unwrap().label, "Leave");
        assert_eq!(d.all_buttons().count(), 2);
    }

    #[test]
    fn the_buttons_answer_carries_what_was_typed() {
        let d = Dialog::read(&login(), &lang());
        let mut form = Form::new(&d);
        form.type_text(&d, 0, "hunter2");
        form.backspace(0);
        form.type_text(&d, 0, "\n\u{7}!");
        let outcome = form.press(&d, &d.buttons[0]);
        assert_eq!(
            outcome,
            Outcome::Send {
                id: "authme:login".into(),
                payload: Some(c([("password", s("hunter!"))])),
            }
        );
        // The way out sends its id, with the action's own (absent) payload.
        assert_eq!(
            form.press(&d, d.exit.as_ref().unwrap()),
            Outcome::Send {
                id: "authme:leave".into(),
                payload: None
            }
        );
    }

    #[test]
    fn a_text_input_stops_at_its_length() {
        let d = Dialog::read(&login(), &lang());
        let mut form = Form::new(&d);
        form.type_text(&d, 0, &"x".repeat(40));
        assert_eq!(form.value(0), Some(&Value::Text("x".repeat(16))));
    }

    #[test]
    fn inputs_have_vanillas_kinds_and_defaults() {
        let tag = c([
            ("type", s("notice")),
            ("title", s("Settings")),
            (
                "inputs",
                Tag::List(vec![
                    c([
                        ("type", s("boolean")),
                        ("key", s("remember")),
                        ("label", s("Remember me")),
                        ("initial", Tag::Byte(1)),
                    ]),
                    c([
                        ("type", s("single_option")),
                        ("key", s("lang")),
                        ("label", s("Language")),
                        (
                            "options",
                            Tag::List(vec![
                                c([("id", s("en")), ("display", s("English"))]),
                                c([
                                    ("id", s("es")),
                                    ("display", s("Español")),
                                    ("initial", Tag::Byte(1)),
                                ]),
                            ]),
                        ),
                    ]),
                    c([
                        ("type", s("number_range")),
                        ("key", s("volume")),
                        ("label", s("Volume")),
                        ("start", Tag::Float(0.0f32.to_bits())),
                        ("end", Tag::Float(10.0f32.to_bits())),
                        ("initial", Tag::Float(5.0f32.to_bits())),
                        ("step", Tag::Float(2.0f32.to_bits())),
                    ]),
                    c([("type", s("unheard_of")), ("key", s("x"))]),
                    c([("type", s("text")), ("label", s("no key"))]),
                ]),
            ),
        ]);
        let d = Dialog::read(&tag, &lang());
        assert_eq!(
            d.inputs.len(),
            3,
            "an unknown input and one without a key are dropped"
        );
        let mut form = Form::new(&d);
        assert_eq!(form.value(0), Some(&Value::Bool(true)));
        assert_eq!(form.value(1), Some(&Value::Choice(1)));
        assert_eq!(form.value(2), Some(&Value::Range(5.0)));
        form.toggle(&d, 0);
        form.toggle(&d, 1);
        form.set_range(&d, 2, 0.83);
        assert_eq!(form.value(0), Some(&Value::Bool(false)));
        assert_eq!(form.value(1), Some(&Value::Choice(0)));
        assert_eq!(form.value(2), Some(&Value::Range(8.0)), "on its steps");
        assert_eq!(
            form.answer(&d),
            c([
                ("remember", Tag::Byte(0)),
                ("lang", s("en")),
                ("volume", Tag::Float(8.0f32.to_bits())),
            ])
        );
        // A notice with no action has an OK button that does nothing.
        assert_eq!(d.kind, Kind::Notice);
        assert_eq!(d.buttons.len(), 1);
        assert_eq!(d.buttons[0].label, "OK");
        assert_eq!(form.press(&d, &d.buttons[0]), Outcome::Nothing);
    }

    #[test]
    fn a_confirmation_has_its_two_buttons_and_a_dialog_can_open_another() {
        let inner = c([("type", s("notice")), ("title", s("Rules"))]);
        let tag = c([
            ("type", s("confirmation")),
            ("title", s("Join?")),
            (
                "yes",
                c([
                    ("label", s("Yes")),
                    (
                        "action",
                        c([("type", s("show_dialog")), ("dialog", inner.clone())]),
                    ),
                ]),
            ),
            (
                "no",
                c([
                    ("label", s("No")),
                    (
                        "action",
                        c([("type", s("run_command")), ("command", s("leave"))]),
                    ),
                ]),
            ),
        ]);
        let d = Dialog::read(&tag, &lang());
        let form = Form::new(&d);
        assert_eq!(d.buttons.len(), 2);
        assert_eq!(form.press(&d, &d.buttons[0]), Outcome::Show(inner));
        assert_eq!(
            form.press(&d, &d.buttons[1]),
            Outcome::RunCommand("leave".into())
        );
    }

    #[test]
    fn a_dynamic_action_adds_its_own_values_to_the_inputs() {
        let tag = c([
            ("type", s("notice")),
            (
                "inputs",
                Tag::List(vec![c([
                    ("type", s("text")),
                    ("key", s("name")),
                    ("initial", s("Ana")),
                ])]),
            ),
            (
                "action",
                c([
                    ("label", s("Send")),
                    (
                        "action",
                        c([
                            ("type", s("dynamic/custom")),
                            ("id", s("a:b")),
                            (
                                "additions",
                                c([("mode", s("login")), ("name", s("ignored"))]),
                            ),
                        ]),
                    ),
                ]),
            ),
        ]);
        let d = Dialog::read(&tag, &lang());
        let outcome = Form::new(&d).press(&d, &d.buttons[0]);
        assert_eq!(
            outcome,
            Outcome::Send {
                id: "a:b".into(),
                payload: Some(c([("name", s("Ana")), ("mode", s("login"))])),
            },
            "what the player typed wins over an addition"
        );
    }

    #[test]
    fn a_dialog_always_has_a_way_out_and_unknown_things_do_not_break_it() {
        // A server-links dialog without an exit action, and a type nobody knows.
        for kind in ["server_links", "something_new"] {
            let d = Dialog::read(&c([("type", s(kind)), ("title", s("T"))]), &lang());
            assert!(d.all_buttons().count() >= 1, "{kind}");
        }
        // An empty or broken value is an empty dialog, not a failure.
        let d = Dialog::read(&Tag::End, &lang());
        assert!(matches!(d.kind, Kind::Other(_)));
        assert!(d.all_buttons().count() >= 1);
        // Unsupported actions do nothing when pressed.
        let tag = c([
            ("type", s("notice")),
            (
                "action",
                c([
                    ("label", s("x")),
                    (
                        "action",
                        c([("type", s("change_page")), ("page", Tag::Int(2))]),
                    ),
                ]),
            ),
        ]);
        let d = Dialog::read(&tag, &lang());
        assert_eq!(Form::new(&d).press(&d, &d.buttons[0]), Outcome::Nothing);
    }

    #[test]
    fn a_dialog_list_has_a_button_for_each_dialog() {
        let tag = c([
            ("type", s("dialog_list")),
            (
                "dialogs",
                Tag::List(vec![
                    c([("type", s("notice")), ("title", s("One"))]),
                    c([("type", s("notice")), ("title", s("Two"))]),
                ]),
            ),
            ("button_width", Tag::Int(90)),
        ]);
        let d = Dialog::read(&tag, &lang());
        let labels: Vec<_> = d.buttons.iter().map(|b| b.label.as_str()).collect();
        assert_eq!(labels, ["One", "Two"]);
        assert!(d.buttons.iter().all(|b| b.width == 90.0));
    }

    #[test]
    fn after_action_and_titles_in_the_players_language() {
        let mut language = lang();
        language.insert("dialog.login".into(), "Iniciar sesión".into());
        let tag = c([
            ("type", s("notice")),
            ("title", c([("translate", s("dialog.login"))])),
            ("after_action", s("wait_for_response")),
        ]);
        let d = Dialog::read(&tag, &language);
        assert_eq!(d.title, "Iniciar sesión");
        assert_eq!(d.after, After::Wait);
    }
}
