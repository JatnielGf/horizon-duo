//! The dialog packets: a server shows a dialog (a screen made of text, inputs and buttons) and the
//! client answers with the action of the button that was pressed. AuthMe 6 logs players in with
//! one, before they join and after.
//!
//! The shape of a dialog is in `Tag` form here; the client reads it (`horizon_client::dialog`).
use crate::{
    Result,
    buf::{Codec, Ident, Nbt, Opt, Reader, Writer},
    packet,
};
use horizon_world::nbt::Tag;

/// `Holder<Dialog>`: either the dialog's id in the registry the server sent (read as `id + 1`),
/// or `0` and the dialog itself as NBT.
#[derive(Clone, Debug, PartialEq)]
pub enum DialogRef {
    Registry(i32),
    Inline(Tag),
}

pub struct HolderDialog;

impl Codec<DialogRef> for HolderDialog {
    fn read(r: &mut Reader) -> Result<DialogRef> {
        match r.varint()? {
            0 => Ok(DialogRef::Inline(r.nbt()?)),
            id => Ok(DialogRef::Registry(id - 1)),
        }
    }
    fn write(v: &DialogRef, w: &mut Writer) {
        match v {
            DialogRef::Registry(id) => w.varint(id + 1),
            DialogRef::Inline(tag) => {
                w.varint(0);
                w.nbt(tag);
            }
        }
    }
}

packet!(
    /// ClientboundShowDialogPacket (configuration and play).
    ShowDialog { dialog: DialogRef => HolderDialog }
);
packet!(
    /// ServerboundCustomClickActionPacket (configuration and play): the id of a button's custom
    /// action, with the values of the dialog's inputs (or the action's own payload).
    CustomClickAction { id: String => Ident, payload: Option<Tag> => Opt<Nbt> }
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packets::{Empty, configuration as cfg, play};

    fn text(s: &str) -> Tag {
        Tag::String(s.into())
    }

    fn sample() -> Tag {
        Tag::compound([("type", text("minecraft:notice")), ("title", text("Login"))])
    }

    #[test]
    fn a_dialog_is_inline_or_a_registry_entry() {
        for dialog in [DialogRef::Inline(sample()), DialogRef::Registry(7)] {
            let mut w = Writer::new();
            HolderDialog::write(&dialog, &mut w);
            let mut r = Reader::new(&w.data);
            assert_eq!(HolderDialog::read(&mut r).unwrap(), dialog);
            assert_eq!(r.remaining(), 0);
        }
        // Registry ids are sent plus one: 0 is "inline".
        let mut w = Writer::new();
        HolderDialog::write(&DialogRef::Registry(0), &mut w);
        assert_eq!(w.data, [1]);
    }

    #[test]
    fn show_dialog_is_read_in_configuration_and_in_play() {
        let packet = ShowDialog {
            dialog: DialogRef::Inline(sample()),
        };
        let mut w = Writer::new();
        ShowDialog::write(&packet, &mut w);
        let cfg_packet =
            cfg::Clientbound::decode(crate::ids::configuration::clientbound::SHOW_DIALOG, &w.data)
                .unwrap();
        assert_eq!(cfg_packet, cfg::Clientbound::ShowDialog(packet.clone()));
        let play_packet =
            play::Clientbound::decode(crate::ids::play::clientbound::SHOW_DIALOG, &w.data).unwrap();
        assert_eq!(play_packet, play::Clientbound::ShowDialog(packet));
    }

    #[test]
    fn clear_dialog_is_read_in_play_too() {
        let packet =
            play::Clientbound::decode(crate::ids::play::clientbound::CLEAR_DIALOG, &[]).unwrap();
        assert_eq!(packet, play::Clientbound::ClearDialog(Empty));
    }

    #[test]
    fn the_answer_to_a_button_goes_out_in_both_states_with_its_own_id() {
        let click = CustomClickAction {
            id: "authme:login".into(),
            payload: Some(Tag::compound([("password", text("hunter2"))])),
        };
        let configuration = cfg::Serverbound::CustomClickAction(click.clone()).encode();
        assert_eq!(
            configuration[0],
            crate::ids::configuration::serverbound::CUSTOM_CLICK_ACTION as u8
        );
        let playing = play::Serverbound::CustomClickAction(click.clone()).encode();
        assert_eq!(
            playing[0],
            crate::ids::play::serverbound::CUSTOM_CLICK_ACTION as u8
        );
        // The body is the same, and an answer without values is a single "absent" byte.
        assert_eq!(configuration[1..], playing[1..]);
        let none = CustomClickAction {
            id: "a:b".into(),
            payload: None,
        };
        let mut w = Writer::new();
        CustomClickAction::write(&none, &mut w);
        assert_eq!(*w.data.last().unwrap(), 0);
        let mut r = Reader::new(&w.data);
        assert_eq!(CustomClickAction::read(&mut r).unwrap(), none);
    }
}
