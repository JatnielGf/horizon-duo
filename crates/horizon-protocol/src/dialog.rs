//! The dialog packets: a server shows a dialog (a screen made of text, inputs and buttons) and the
//! client answers with the action of the button that was pressed. AuthMe 6 logs players in with
//! one, before they join and after.
//!
//! The shape of a dialog is in `Tag` form here; the client reads it (`horizon_client::dialog`).
use crate::{
    Error, Result,
    buf::{Codec, Ident, Reader, Writer},
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
        // The dialog is the whole of the packet, so every layout is tried on all of it and the
        // one that uses it exactly is the one.
        let all = r.rest();
        layouts(all).ok_or_else(|| {
            let head: Vec<String> = all.iter().take(40).map(|b| format!("{b:02x}")).collect();
            Error::new(format!(
                "dialog of {} bytes not understood, starting {}",
                all.len(),
                head.join(" ")
            ))
        })
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

/// The ways a dialog has been seen on the wire: vanilla's (`0`, then the dialog as NBT, or a
/// registry id plus one alone), the dialog as NBT without the holder in front, and `0` then NBT
/// whose root has a name.
fn layouts(all: &[u8]) -> Option<DialogRef> {
    let mut r = Reader::new(all);
    if let Ok(id) = r.varint() {
        if id == 0 {
            if let Ok(tag) = r.nbt()
                && tag != Tag::End
                && r.remaining() == 0
            {
                return Some(DialogRef::Inline(tag));
            }
        } else if r.remaining() == 0 {
            return Some(DialogRef::Registry(id - 1));
        }
    }
    // No holder in front.
    let mut r = Reader::new(all);
    if let Ok(tag) = r.nbt()
        && tag != Tag::End
        && r.remaining() == 0
    {
        return Some(DialogRef::Inline(tag));
    }
    // A holder `0`, then a root with a name: its type, the name, then the payload.
    if all.len() > 4 && all[0] == 0 && all[1] != 0 {
        let name_len = usize::from(u16::from_be_bytes([all[2], all[3]]));
        if let Some(payload) = all.get(4 + name_len..) {
            let mut bytes = vec![all[1]];
            bytes.extend_from_slice(payload);
            let mut r = Reader::new(&bytes);
            if let Ok(tag) = r.nbt()
                && tag != Tag::End
                && r.remaining() == 0
            {
                return Some(DialogRef::Inline(tag));
            }
        }
    }
    None
}

packet!(
    /// ClientboundShowDialogPacket (configuration and play).
    ShowDialog { dialog: DialogRef => HolderDialog }
);
/// `ByteBufCodecs.optionalTagCodec`: a tag, or one `End` byte for none. There is no presence flag
/// in front, unlike `ByteBufCodecs.optional`; a server reads a flag as the type of the tag.
pub struct OptionalTag;

impl Codec<Option<Tag>> for OptionalTag {
    fn read(r: &mut Reader) -> Result<Option<Tag>> {
        Ok(match r.nbt()? {
            Tag::End => None,
            tag => Some(tag),
        })
    }
    fn write(v: &Option<Tag>, w: &mut Writer) {
        w.nbt(v.as_ref().unwrap_or(&Tag::End));
    }
}

packet!(
    /// ServerboundCustomClickActionPacket (configuration and play): the id of a button's custom
    /// action, with the values of the dialog's inputs (or the action's own payload).
    CustomClickAction { id: String => Ident, payload: Option<Tag> => OptionalTag }
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
    fn other_layouts_of_a_dialog_are_read_too() {
        let tag = sample();
        let mut nbt = Writer::new();
        nbt.nbt(&tag);
        // The dialog alone, with no holder in front.
        let mut r = Reader::new(&nbt.data);
        assert_eq!(
            HolderDialog::read(&mut r).unwrap(),
            DialogRef::Inline(tag.clone())
        );
        // A holder `0`, then a root with a name.
        let mut named = vec![0, nbt.data[0], 0, 4];
        named.extend_from_slice(b"root");
        named.extend_from_slice(&nbt.data[1..]);
        let mut r = Reader::new(&named);
        assert_eq!(HolderDialog::read(&mut r).unwrap(), DialogRef::Inline(tag));
    }

    #[test]
    fn a_dialog_nobody_understands_says_how_it_starts() {
        let data = [0u8, 0x42, 0x13, 0x37, 0x99, 0x01];
        let mut r = Reader::new(&data);
        let error = HolderDialog::read(&mut r).unwrap_err().to_string();
        assert!(error.contains("6 bytes"), "{error}");
        assert!(error.contains("00 42 13 37 99 01"), "{error}");
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
        let mut r = Reader::new(&w.data);
        assert_eq!(CustomClickAction::read(&mut r).unwrap(), none);
    }

    #[test]
    fn the_answer_is_laid_out_the_way_a_server_reads_it() {
        // The id as a string, then the values as a tag with nothing in between: a server reads
        // the byte after the id as the type of the tag, so a presence flag there breaks it.
        let mut w = Writer::new();
        CustomClickAction::write(
            &CustomClickAction {
                id: "a:b".into(),
                payload: None,
            },
            &mut w,
        );
        assert_eq!(w.data, [3, b'a', b':', b'b', 0], "no values: one End byte");
        let mut w = Writer::new();
        CustomClickAction::write(
            &CustomClickAction {
                id: "a:b".into(),
                payload: Some(Tag::compound([("k", text("v"))])),
            },
            &mut w,
        );
        assert_eq!(
            w.data,
            [
                3, b'a', b':', b'b', // the id
                10,   // a compound, with no name
                8, 0, 1, b'k', 0, 1, b'v', // "k": "v"
                0,    // its end
            ]
        );
    }
}
