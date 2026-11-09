//! A server's dialog (`ClientboundShowDialogPacket`) bound to the game: the screen it shows over
//! everything, in the connecting phase or in a world, and the answer it sends. AuthMe 6 logs
//! players in with one. Like the resource pack prompt it covers the world and takes the mouse; it
//! also takes the keyboard for its text inputs.
use super::Game;
use horizon_client::{
    dialog::{Action, After, Control, Dialog, Form, Outcome, Value},
    dialog_ui::{self, Hit, Placed},
    hud::{HudAtlas, HudBatch, rgba},
    text::Text,
    ui, widgets,
};
use horizon_protocol::{dialog::CustomClickAction, dialog::DialogRef};
use horizon_world::nbt::Tag;
use std::time::Instant;

/// A dialog being shown, and what the player has put into it.
pub(super) struct ActiveDialog {
    dialog: Dialog,
    form: Form,
    /// The text input that takes the keyboard.
    focus: Option<usize>,
    /// Pressed, and the server hasn't answered yet.
    waiting: bool,
    shown: Instant,
}

impl ActiveDialog {
    fn new(dialog: Dialog) -> Self {
        let focus = dialog
            .inputs
            .iter()
            .position(|input| matches!(input.control, Control::Text { .. }));
        Self {
            form: Form::new(&dialog),
            dialog,
            focus,
            waiting: false,
            shown: Instant::now(),
        }
    }

    fn placed(&self, atlas: &HudAtlas, size: (f32, f32)) -> Placed {
        dialog_ui::place(&self.dialog, size, &|text| {
            HudBatch::text_width(atlas, text)
        })
    }
}

/// What a control shows as its label on a button.
fn state_label(input: &horizon_client::dialog::Input, value: Option<&Value>) -> String {
    let shown = match (&input.control, value) {
        (
            Control::Boolean {
                on_true, on_false, ..
            },
            Some(Value::Bool(on)),
        ) => {
            if *on {
                on_true.clone()
            } else {
                on_false.clone()
            }
        }
        (Control::Choice { options, .. }, Some(Value::Choice(at))) => options
            .get(*at)
            .map(|(_, shown)| shown.clone())
            .unwrap_or_default(),
        (Control::Range { .. }, Some(Value::Range(v))) => {
            if v.fract() == 0.0 {
                format!("{v:.0}")
            } else {
                format!("{v:.2}")
            }
        }
        _ => String::new(),
    };
    if input.label.is_empty() {
        shown
    } else {
        format!("{}: {shown}", input.label)
    }
}

impl Game<'_> {
    pub(super) fn dialog_open(&self) -> bool {
        self.remote.as_ref().is_some_and(|r| r.dialog.is_some())
    }

    /// Whether a text input of the dialog has the keyboard.
    pub(super) fn dialog_text_focused(&self) -> bool {
        self.remote
            .as_ref()
            .and_then(|r| r.dialog.as_ref())
            .is_some_and(|d| d.focus.is_some() && !d.waiting)
    }

    /// How many characters the focused text input holds.
    pub(super) fn dialog_text_len(&self) -> usize {
        self.remote
            .as_ref()
            .and_then(|r| r.dialog.as_ref())
            .and_then(|d| d.focus.and_then(|i| d.form.value(i)))
            .map_or(0, |value| match value {
                Value::Text(text) => text.chars().count(),
                _ => 0,
            })
    }

    /// Shows the dialog in `tag`, in place of the one shown.
    pub(super) fn show_dialog(&mut self, tag: &Tag) {
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        log::debug!("Horizon: server dialog data: {tag:?}");
        let dialog = Dialog::read(tag, &r.lang);
        log::info!(
            "Horizon: server dialog \"{}\": {} inputs, {} buttons",
            dialog.title,
            dialog.inputs.len(),
            dialog.all_buttons().count()
        );
        r.dialog = Some(ActiveDialog::new(dialog));
        self.capture(false);
    }

    /// The same, for a dialog the server gives by its id in the registries it sent.
    pub(super) fn show_dialog_ref(&mut self, dialog: DialogRef) {
        let tag = match dialog {
            DialogRef::Inline(tag) => Some(tag),
            DialogRef::Registry(id) => self
                .remote
                .as_ref()
                .and_then(|r| r.registries.data("minecraft:dialog", id).cloned()),
        };
        match tag {
            Some(tag) => self.show_dialog(&tag),
            None => log::warn!("Horizon: the server showed a dialog this client has no entry for"),
        }
    }

    /// ClientboundClearDialogPacket, or the player closing it.
    pub(super) fn clear_dialog(&mut self) {
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        if r.dialog.take().is_some()
            && self.player.is_some()
            && self.screen == super::Screen::Playing
        {
            self.capture(true);
        }
    }

    pub(super) fn draw_dialog(&self, batch: &mut HudBatch) {
        let Some(active) = self.remote.as_ref().and_then(|r| r.dialog.as_ref()) else {
            return;
        };
        let atlas = &self.gui.atlas;
        let (w, h) = (batch.width, batch.height);
        if self.player.is_none() {
            // In place of the connecting screen, on its background.
            batch.fill(atlas, 0.0, 0.0, w, h, rgba(20, 20, 20, 255));
        }
        ui::menu_background(batch, atlas, 0.0, 0.0, w, h, self.player.is_some());
        let placed = active.placed(atlas, (w, h));
        let white = u32::MAX;
        let gray = rgba(170, 170, 170, 255);
        let dialog = &active.dialog;
        let cursor = self.cursor;
        if let Some((title, y)) = &placed.title {
            let width = HudBatch::text_width(atlas, title);
            batch.text(
                atlas,
                title,
                placed.center_x - (width / 2.0).floor(),
                *y,
                white,
                true,
            );
        }
        for (line, y) in &placed.lines {
            let width = HudBatch::text_width(atlas, line);
            batch.text(
                atlas,
                line,
                placed.center_x - (width / 2.0).floor(),
                *y,
                gray,
                true,
            );
        }
        if placed.cut {
            batch.text(atlas, "...", placed.center_x - 9.0, h - 10.0, gray, true);
        }
        for (i, (input, spot)) in dialog.inputs.iter().zip(&placed.inputs).enumerate() {
            if let Some((label, y)) = &spot.label {
                batch.text(atlas, label, spot.rect[0], *y, gray, true);
            }
            let [x, y, width, height] = spot.rect;
            match &input.control {
                Control::Text { .. } => {
                    let focused = active.focus == Some(i) && !active.waiting;
                    // A tap on a text field asks for the keyboard.
                    widgets::target_as(spot.rect, widgets::Kind::Field);
                    let sprite = if focused {
                        "gui/sprites/widget/text_field_highlighted"
                    } else {
                        "gui/sprites/widget/text_field"
                    };
                    ui::nine_slice(batch, atlas, sprite, x, y, width, height, white);
                    let value = match active.form.value(i) {
                        Some(Value::Text(text)) => text.as_str(),
                        _ => "",
                    };
                    // The end of what fits is the part that shows.
                    let room = width - 8.0;
                    let mut shown = value;
                    while HudBatch::text_width(atlas, shown) > room {
                        let mut chars = shown.chars();
                        chars.next();
                        shown = chars.as_str();
                    }
                    batch.text(atlas, shown, x + 4.0, y + (height - 8.0) / 2.0, white, true);
                    if focused && (active.shown.elapsed().as_millis() / 500) % 2 == 0 {
                        let caret = x + 4.0 + HudBatch::text_width(atlas, shown);
                        batch.fill(atlas, caret, y + 5.0, 1.0, height - 10.0, white);
                    }
                }
                _ => {
                    let label = Text::literal(state_label(input, active.form.value(i)));
                    let hover = ui::hovered(cursor, x, y, width, height) && !active.waiting;
                    ui::button(
                        batch,
                        atlas,
                        x,
                        y,
                        width,
                        height,
                        &label,
                        !active.waiting,
                        hover,
                    );
                }
            }
        }
        for (button, rect) in dialog.all_buttons().zip(&placed.buttons) {
            let [x, y, width, height] = *rect;
            let label = Text::literal(button.label.clone());
            let hover = ui::hovered(cursor, x, y, width, height) && !active.waiting;
            ui::button(
                batch,
                atlas,
                x,
                y,
                width,
                height,
                &label,
                !active.waiting,
                hover,
            );
        }
        if active.waiting {
            let text = "Waiting for the server...";
            let width = HudBatch::text_width(atlas, text);
            batch.text(
                atlas,
                text,
                placed.center_x - (width / 2.0).floor(),
                h - 14.0,
                gray,
                true,
            );
        }
    }

    /// A left click on the dialog: what it hit.
    pub(super) fn dialog_click(&mut self) {
        let size = self.gui_size();
        let cursor = self.cursor;
        let Some(active) = self.remote.as_ref().and_then(|r| r.dialog.as_ref()) else {
            return;
        };
        if active.waiting {
            return;
        }
        let placed = active.placed(&self.gui.atlas, size);
        match placed.hit(cursor.0, cursor.1) {
            Some(Hit::Button(n)) => {
                self.audio.play_button_click();
                self.press_dialog_button(n);
            }
            Some(Hit::Input(i)) => {
                let rect = placed.inputs[i].rect;
                let Some(active) = self.remote.as_mut().and_then(|r| r.dialog.as_mut()) else {
                    return;
                };
                let Active {
                    dialog,
                    form,
                    focus,
                    ..
                } = Active::split(active);
                match dialog.inputs.get(i).map(|input| &input.control) {
                    Some(Control::Text { .. }) => {
                        *focus = Some(i);
                        self.request_keyboard();
                    }
                    Some(Control::Range { .. }) => {
                        let share = (cursor.0 - rect[0]) / rect[2].max(1.0);
                        form.set_range(dialog, i, share);
                    }
                    Some(_) => {
                        form.toggle(dialog, i);
                        self.audio.play_button_click();
                    }
                    None => {}
                }
            }
            None => {}
        }
    }

    /// Presses button `n` (the way out last), and does what it asks for.
    fn press_dialog_button(&mut self, n: usize) {
        let Some(active) = self.remote.as_ref().and_then(|r| r.dialog.as_ref()) else {
            return;
        };
        let Some(button) = active.dialog.all_buttons().nth(n) else {
            return;
        };
        let outcome = active.form.press(&active.dialog, button);
        let after = active.dialog.after;
        let replaced = matches!(outcome, Outcome::Show(_));
        match outcome {
            Outcome::Nothing => {}
            Outcome::Send { id, payload } => {
                if let Some(r) = &self.remote {
                    r.session
                        .send_dialog_click(CustomClickAction { id, payload });
                }
            }
            Outcome::Show(tag) => self.show_dialog(&tag),
            Outcome::RunCommand(command) => {
                if self.player.is_some() {
                    self.send_chat(&format!("/{command}"));
                }
            }
            Outcome::SuggestCommand(_) | Outcome::OpenUrl(_) | Outcome::CopyToClipboard(_) => {
                log::info!(
                    "Horizon: a dialog button asked for something this client doesn't do yet"
                );
            }
        }
        if replaced {
            return;
        }
        match after {
            After::Close => self.clear_dialog(),
            After::Wait => {
                if let Some(a) = self.remote.as_mut().and_then(|r| r.dialog.as_mut()) {
                    a.waiting = true;
                }
            }
            After::Nothing => {}
        }
    }

    /// Escape: closes a dialog that can be closed.
    pub(super) fn dialog_escape(&mut self) {
        let closes = self
            .remote
            .as_ref()
            .and_then(|r| r.dialog.as_ref())
            .is_some_and(|d| d.dialog.escape_closes && !d.waiting);
        if closes {
            self.clear_dialog();
        }
    }

    /// Typed text goes into the input that has the keyboard.
    pub(super) fn dialog_type(&mut self, text: &str) {
        if let Some(active) = self.remote.as_mut().and_then(|r| r.dialog.as_mut()) {
            let Active {
                dialog,
                form,
                focus,
                waiting,
            } = Active::split(active);
            if let (Some(i), false) = (*focus, *waiting) {
                form.type_text(dialog, i, text);
            }
        }
    }

    pub(super) fn dialog_backspace(&mut self) {
        if let Some(active) = self.remote.as_mut().and_then(|r| r.dialog.as_mut())
            && let (Some(i), false) = (active.focus, active.waiting)
        {
            active.form.backspace(i);
        }
    }

    /// Enter: the first button, as a login form's Enter would.
    pub(super) fn dialog_enter(&mut self) {
        let sendable = self
            .remote
            .as_ref()
            .and_then(|r| r.dialog.as_ref())
            .is_some_and(|d| {
                !d.waiting
                    && d.dialog
                        .buttons
                        .first()
                        .is_some_and(|b| matches!(b.action, Action::Custom { .. }))
            });
        if sendable {
            self.press_dialog_button(0);
        }
    }

    /// Tab: the next text input.
    pub(super) fn dialog_next_input(&mut self) {
        let Some(active) = self.remote.as_mut().and_then(|r| r.dialog.as_mut()) else {
            return;
        };
        let texts: Vec<usize> = active
            .dialog
            .inputs
            .iter()
            .enumerate()
            .filter(|(_, i)| matches!(i.control, Control::Text { .. }))
            .map(|(n, _)| n)
            .collect();
        if texts.len() > 1 {
            let at = texts
                .iter()
                .position(|n| Some(*n) == active.focus)
                .unwrap_or(0);
            active.focus = Some(texts[(at + 1) % texts.len()]);
        }
    }
}

/// The parts of an [`ActiveDialog`] a click changes, borrowed together.
struct Active<'a> {
    dialog: &'a Dialog,
    form: &'a mut Form,
    focus: &'a mut Option<usize>,
    waiting: &'a mut bool,
}

impl<'a> Active<'a> {
    fn split(active: &'a mut ActiveDialog) -> Self {
        Self {
            dialog: &active.dialog,
            form: &mut active.form,
            focus: &mut active.focus,
            waiting: &mut active.waiting,
        }
    }
}
