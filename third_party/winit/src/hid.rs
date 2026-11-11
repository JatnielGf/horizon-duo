//! USB HID keyboard usages (page 0x07) as winit's keys. iOS' GameController reports a hardware
//! keyboard's keys by these numbers (`GCKeyCode`), and so does UIKit (`UIKeyboardHIDUsage`).
//! Kept apart from the platform code so it is built, and tested, everywhere.
#![cfg_attr(not(target_os = "ios"), allow(dead_code))]

use smol_str::SmolStr;

use crate::keyboard::{Key, KeyCode, KeyLocation, NamedKey, NativeKey};

/// The physical key with this HID usage, if winit has one.
pub(crate) fn key_code(usage: u32) -> Option<KeyCode> {
    use KeyCode::*;
    const LETTERS: [KeyCode; 26] = [
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO,
        KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
    ];
    const DIGITS: [KeyCode; 10] = [
        Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9, Digit0,
    ];
    const FUNCTION: [KeyCode; 12] = [F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12];
    const FUNCTION_HIGH: [KeyCode; 12] =
        [F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24];
    const PAD: [KeyCode; 10] = [
        Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9, Numpad0,
    ];
    Some(match usage {
        0x04..=0x1D => LETTERS[(usage - 0x04) as usize],
        0x1E..=0x27 => DIGITS[(usage - 0x1E) as usize],
        0x28 => Enter,
        0x29 => Escape,
        0x2A => Backspace,
        0x2B => Tab,
        0x2C => Space,
        0x2D => Minus,
        0x2E => Equal,
        0x2F => BracketLeft,
        0x30 => BracketRight,
        // The key beside Enter on an ISO keyboard sits where ANSI has the backslash.
        0x31 | 0x32 => Backslash,
        0x33 => Semicolon,
        0x34 => Quote,
        0x35 => Backquote,
        0x36 => Comma,
        0x37 => Period,
        0x38 => Slash,
        0x39 => CapsLock,
        0x3A..=0x45 => FUNCTION[(usage - 0x3A) as usize],
        0x46 => PrintScreen,
        0x47 => ScrollLock,
        0x48 => Pause,
        0x49 => Insert,
        0x4A => Home,
        0x4B => PageUp,
        0x4C => Delete,
        0x4D => End,
        0x4E => PageDown,
        0x4F => ArrowRight,
        0x50 => ArrowLeft,
        0x51 => ArrowDown,
        0x52 => ArrowUp,
        0x53 => NumLock,
        0x54 => NumpadDivide,
        0x55 => NumpadMultiply,
        0x56 => NumpadSubtract,
        0x57 => NumpadAdd,
        0x58 => NumpadEnter,
        0x59..=0x62 => PAD[(usage - 0x59) as usize],
        0x63 => NumpadDecimal,
        0x64 => IntlBackslash,
        0x65 => ContextMenu,
        0x67 => NumpadEqual,
        0x68..=0x73 => FUNCTION_HIGH[(usage - 0x68) as usize],
        0xE0 => ControlLeft,
        0xE1 => ShiftLeft,
        0xE2 => AltLeft,
        0xE3 => SuperLeft,
        0xE4 => ControlRight,
        0xE5 => ShiftRight,
        0xE6 => AltRight,
        0xE7 => SuperRight,
        _ => return None,
    })
}

/// Whether the system's text input takes this key when a text field has the keyboard (typing,
/// Return and Backspace reach the app as `insertText` and `deleteBackward`), so a key event for
/// it would write the same thing twice.
pub(crate) fn is_text_key(code: KeyCode) -> bool {
    use KeyCode::*;
    matches!(
        code,
        KeyA | KeyB | KeyC | KeyD | KeyE | KeyF | KeyG | KeyH | KeyI | KeyJ | KeyK | KeyL
            | KeyM | KeyN | KeyO | KeyP | KeyQ | KeyR | KeyS | KeyT | KeyU | KeyV | KeyW | KeyX
            | KeyY | KeyZ | Digit0 | Digit1 | Digit2 | Digit3 | Digit4 | Digit5 | Digit6
            | Digit7 | Digit8 | Digit9 | Space | Enter | Backspace | Minus | Equal
            | BracketLeft | BracketRight | Backslash | Semicolon | Quote | Backquote | Comma
            | Period | Slash | IntlBackslash | NumpadEnter | NumpadDivide | NumpadMultiply
            | NumpadSubtract | NumpadAdd | NumpadDecimal | NumpadEqual | Numpad0 | Numpad1
            | Numpad2 | Numpad3 | Numpad4 | Numpad5 | Numpad6 | Numpad7 | Numpad8 | Numpad9
    )
}

/// Where on the keyboard the key is.
pub(crate) fn location(code: KeyCode) -> KeyLocation {
    use KeyCode::*;
    match code {
        ControlLeft | ShiftLeft | AltLeft | SuperLeft => KeyLocation::Left,
        ControlRight | ShiftRight | AltRight | SuperRight => KeyLocation::Right,
        NumLock | NumpadDivide | NumpadMultiply | NumpadSubtract | NumpadAdd | NumpadEnter
        | NumpadDecimal | NumpadEqual | Numpad0 | Numpad1 | Numpad2 | Numpad3 | Numpad4
        | Numpad5 | Numpad6 | Numpad7 | Numpad8 | Numpad9 => KeyLocation::Numpad,
        _ => KeyLocation::Standard,
    }
}

/// What the key means, on a US layout (the layout a hardware keyboard's keys are named for).
pub(crate) fn logical_key(code: KeyCode, shift: bool) -> Key {
    use KeyCode::*;
    let character = |plain: &str, shifted: &str| {
        Key::Character(SmolStr::new(if shift { shifted } else { plain }))
    };
    let named = Key::Named;
    match code {
        KeyA => character("a", "A"),
        KeyB => character("b", "B"),
        KeyC => character("c", "C"),
        KeyD => character("d", "D"),
        KeyE => character("e", "E"),
        KeyF => character("f", "F"),
        KeyG => character("g", "G"),
        KeyH => character("h", "H"),
        KeyI => character("i", "I"),
        KeyJ => character("j", "J"),
        KeyK => character("k", "K"),
        KeyL => character("l", "L"),
        KeyM => character("m", "M"),
        KeyN => character("n", "N"),
        KeyO => character("o", "O"),
        KeyP => character("p", "P"),
        KeyQ => character("q", "Q"),
        KeyR => character("r", "R"),
        KeyS => character("s", "S"),
        KeyT => character("t", "T"),
        KeyU => character("u", "U"),
        KeyV => character("v", "V"),
        KeyW => character("w", "W"),
        KeyX => character("x", "X"),
        KeyY => character("y", "Y"),
        KeyZ => character("z", "Z"),
        Digit1 => character("1", "!"),
        Digit2 => character("2", "@"),
        Digit3 => character("3", "#"),
        Digit4 => character("4", "$"),
        Digit5 => character("5", "%"),
        Digit6 => character("6", "^"),
        Digit7 => character("7", "&"),
        Digit8 => character("8", "*"),
        Digit9 => character("9", "("),
        Digit0 => character("0", ")"),
        Minus => character("-", "_"),
        Equal => character("=", "+"),
        BracketLeft => character("[", "{"),
        BracketRight => character("]", "}"),
        Backslash | IntlBackslash => character("\\", "|"),
        Semicolon => character(";", ":"),
        Quote => character("'", "\""),
        Backquote => character("`", "~"),
        Comma => character(",", "<"),
        Period => character(".", ">"),
        Slash => character("/", "?"),
        Numpad0 => character("0", "0"),
        Numpad1 => character("1", "1"),
        Numpad2 => character("2", "2"),
        Numpad3 => character("3", "3"),
        Numpad4 => character("4", "4"),
        Numpad5 => character("5", "5"),
        Numpad6 => character("6", "6"),
        Numpad7 => character("7", "7"),
        Numpad8 => character("8", "8"),
        Numpad9 => character("9", "9"),
        NumpadAdd => character("+", "+"),
        NumpadSubtract => character("-", "-"),
        NumpadMultiply => character("*", "*"),
        NumpadDivide => character("/", "/"),
        NumpadDecimal => character(".", "."),
        NumpadEqual => character("=", "="),
        Space => named(NamedKey::Space),
        Enter | NumpadEnter => named(NamedKey::Enter),
        Escape => named(NamedKey::Escape),
        Backspace => named(NamedKey::Backspace),
        Tab => named(NamedKey::Tab),
        Delete => named(NamedKey::Delete),
        Insert => named(NamedKey::Insert),
        Home => named(NamedKey::Home),
        End => named(NamedKey::End),
        PageUp => named(NamedKey::PageUp),
        PageDown => named(NamedKey::PageDown),
        ArrowUp => named(NamedKey::ArrowUp),
        ArrowDown => named(NamedKey::ArrowDown),
        ArrowLeft => named(NamedKey::ArrowLeft),
        ArrowRight => named(NamedKey::ArrowRight),
        CapsLock => named(NamedKey::CapsLock),
        NumLock => named(NamedKey::NumLock),
        ScrollLock => named(NamedKey::ScrollLock),
        PrintScreen => named(NamedKey::PrintScreen),
        Pause => named(NamedKey::Pause),
        ContextMenu => named(NamedKey::ContextMenu),
        F1 => named(NamedKey::F1),
        F2 => named(NamedKey::F2),
        F3 => named(NamedKey::F3),
        F4 => named(NamedKey::F4),
        F5 => named(NamedKey::F5),
        F6 => named(NamedKey::F6),
        F7 => named(NamedKey::F7),
        F8 => named(NamedKey::F8),
        F9 => named(NamedKey::F9),
        F10 => named(NamedKey::F10),
        F11 => named(NamedKey::F11),
        F12 => named(NamedKey::F12),
        F13 => named(NamedKey::F13),
        F14 => named(NamedKey::F14),
        F15 => named(NamedKey::F15),
        F16 => named(NamedKey::F16),
        F17 => named(NamedKey::F17),
        F18 => named(NamedKey::F18),
        F19 => named(NamedKey::F19),
        F20 => named(NamedKey::F20),
        F21 => named(NamedKey::F21),
        F22 => named(NamedKey::F22),
        F23 => named(NamedKey::F23),
        F24 => named(NamedKey::F24),
        ShiftLeft | ShiftRight => named(NamedKey::Shift),
        ControlLeft | ControlRight => named(NamedKey::Control),
        AltLeft | AltRight => named(NamedKey::Alt),
        SuperLeft | SuperRight => named(NamedKey::Super),
        _ => Key::Unidentified(NativeKey::Unidentified),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_digits_and_the_keys_around_them_have_their_usages() {
        assert_eq!(key_code(0x04), Some(KeyCode::KeyA));
        assert_eq!(key_code(0x1A), Some(KeyCode::KeyW));
        assert_eq!(key_code(0x1D), Some(KeyCode::KeyZ));
        assert_eq!(key_code(0x1E), Some(KeyCode::Digit1));
        assert_eq!(key_code(0x26), Some(KeyCode::Digit9));
        assert_eq!(key_code(0x27), Some(KeyCode::Digit0));
        assert_eq!(key_code(0x28), Some(KeyCode::Enter));
        assert_eq!(key_code(0x29), Some(KeyCode::Escape));
        assert_eq!(key_code(0x2A), Some(KeyCode::Backspace));
        assert_eq!(key_code(0x2B), Some(KeyCode::Tab));
        assert_eq!(key_code(0x2C), Some(KeyCode::Space));
        assert_eq!(key_code(0x35), Some(KeyCode::Backquote));
    }

    #[test]
    fn function_arrow_numpad_and_modifier_keys_are_found() {
        assert_eq!(key_code(0x3A), Some(KeyCode::F1));
        assert_eq!(key_code(0x45), Some(KeyCode::F12));
        assert_eq!(key_code(0x68), Some(KeyCode::F13));
        assert_eq!(key_code(0x73), Some(KeyCode::F24));
        assert_eq!(key_code(0x4F), Some(KeyCode::ArrowRight));
        assert_eq!(key_code(0x50), Some(KeyCode::ArrowLeft));
        assert_eq!(key_code(0x51), Some(KeyCode::ArrowDown));
        assert_eq!(key_code(0x52), Some(KeyCode::ArrowUp));
        assert_eq!(key_code(0x59), Some(KeyCode::Numpad1));
        assert_eq!(key_code(0x62), Some(KeyCode::Numpad0));
        assert_eq!(key_code(0x58), Some(KeyCode::NumpadEnter));
        assert_eq!(key_code(0xE0), Some(KeyCode::ControlLeft));
        assert_eq!(key_code(0xE1), Some(KeyCode::ShiftLeft));
        assert_eq!(key_code(0xE2), Some(KeyCode::AltLeft));
        assert_eq!(key_code(0xE3), Some(KeyCode::SuperLeft));
        assert_eq!(key_code(0xE5), Some(KeyCode::ShiftRight));
        assert_eq!(key_code(0xE7), Some(KeyCode::SuperRight));
    }

    #[test]
    fn what_is_not_a_key_is_not_found() {
        for usage in [0, 1, 2, 3, 0x66, 0x74, 0xAF, 0xE8, 0xFFFF] {
            assert_eq!(key_code(usage), None, "{usage:#x}");
        }
    }

    #[test]
    fn every_usage_in_the_ranges_maps_to_a_different_key() {
        let mut seen = std::collections::HashSet::new();
        for usage in 0x04..=0x73u32 {
            if let Some(code) = key_code(usage) {
                // 0x31 and 0x32 are both the backslash position on different keyboards.
                if usage != 0x32 {
                    assert!(seen.insert(code), "{usage:#x} repeats {code:?}");
                }
            }
        }
        assert!(seen.len() > 100);
    }

    #[test]
    fn the_text_system_keeps_what_it_types_and_the_game_keeps_the_rest() {
        for code in [KeyCode::KeyW, KeyCode::Digit5, KeyCode::Space, KeyCode::Enter, KeyCode::Backspace, KeyCode::Comma] {
            assert!(is_text_key(code), "{code:?}");
        }
        for code in [
            KeyCode::Escape,
            KeyCode::Tab,
            KeyCode::ArrowUp,
            KeyCode::F3,
            KeyCode::ShiftLeft,
            KeyCode::ControlLeft,
            KeyCode::Delete,
            KeyCode::CapsLock,
        ] {
            assert!(!is_text_key(code), "{code:?}");
        }
    }

    #[test]
    fn keys_mean_what_they_say_on_a_us_layout() {
        assert_eq!(logical_key(KeyCode::KeyA, false), Key::Character("a".into()));
        assert_eq!(logical_key(KeyCode::KeyA, true), Key::Character("A".into()));
        assert_eq!(logical_key(KeyCode::Digit1, true), Key::Character("!".into()));
        assert_eq!(logical_key(KeyCode::Slash, true), Key::Character("?".into()));
        assert_eq!(logical_key(KeyCode::Numpad7, true), Key::Character("7".into()));
        assert_eq!(logical_key(KeyCode::Escape, false), Key::Named(NamedKey::Escape));
        assert_eq!(logical_key(KeyCode::Space, false), Key::Named(NamedKey::Space));
        assert_eq!(logical_key(KeyCode::ShiftRight, false), Key::Named(NamedKey::Shift));
        assert_eq!(logical_key(KeyCode::F11, false), Key::Named(NamedKey::F11));
        assert!(matches!(logical_key(KeyCode::Lang1, false), Key::Unidentified(_)));
    }

    #[test]
    fn modifiers_and_the_numpad_have_a_side() {
        assert_eq!(location(KeyCode::ShiftLeft), KeyLocation::Left);
        assert_eq!(location(KeyCode::AltRight), KeyLocation::Right);
        assert_eq!(location(KeyCode::Numpad3), KeyLocation::Numpad);
        assert_eq!(location(KeyCode::KeyQ), KeyLocation::Standard);
    }
}
