//! A hardware keyboard, through GameController. UIKit hands key presses to the first responder,
//! and the view is that only while a text field has the keyboard; GameController reports the
//! keys of a keyboard that is plugged in, paired or (on an iPad) folded to it whatever the view
//! is doing, which is what a game needs.
//!
//! Text is not this module's: while the view has the keyboard, UIKit sends what is typed as
//! `insertText` and `deleteBackward`, so the keys that write text are left out then (see
//! `hid::is_text_key`) and every other key still reaches the app.
use std::cell::RefCell;
use std::collections::HashSet;
use std::ptr::NonNull;
use std::rc::Rc;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_foundation::{MainThreadMarker, NSNotificationCenter};
use objc2_game_controller::{
    GCControllerButtonInput, GCKeyboard, GCKeyboardDidConnectNotification,
    GCKeyboardDidDisconnectNotification, GCKeyboardInput,
};

use super::notification_center::create_observer;
use super::view::WinitView;
use crate::event::{ElementState, KeyEvent, Modifiers, WindowEvent};
use crate::hid;
use crate::keyboard::{ModifiersState, PhysicalKey};
use crate::platform_impl::platform::DEVICE_ID;
use crate::platform_impl::KeyEventExtra;

/// The keys the app has been told are down, by their HID usage.
#[derive(Default)]
struct Keys {
    down: HashSet<isize>,
    modifiers: ModifiersState,
}

impl Keys {
    /// The modifiers the keys that are down add up to.
    fn modifiers_down(&self) -> ModifiersState {
        let has = |usages: [isize; 2]| usages.iter().any(|u| self.down.contains(u));
        let mut state = ModifiersState::empty();
        state.set(ModifiersState::CONTROL, has([0xE0, 0xE4]));
        state.set(ModifiersState::SHIFT, has([0xE1, 0xE5]));
        state.set(ModifiersState::ALT, has([0xE2, 0xE6]));
        state.set(ModifiersState::SUPER, has([0xE3, 0xE7]));
        state
    }
}

/// Starts listening to the hardware keyboard for `view`: the one connected now, and any that
/// connects later.
pub(crate) fn attach(view: &Retained<WinitView>) {
    let keys = Rc::new(RefCell::new(Keys::default()));
    connect(view, &keys);

    let center = unsafe { NSNotificationCenter::defaultCenter() };
    let (connecting, shared) = (view.clone(), keys.clone());
    let connected = create_observer(&center, unsafe { GCKeyboardDidConnectNotification }, move |_| {
        connect(&connecting, &shared)
    });
    // A keyboard that goes away takes its releases with it: the keys it had down are let go.
    let (leaving, shared) = (view.clone(), keys);
    let disconnected = create_observer(
        &center,
        unsafe { GCKeyboardDidDisconnectNotification },
        move |_| release_all(&leaving, &shared),
    );
    // The observers last as long as the app does.
    std::mem::forget(connected);
    std::mem::forget(disconnected);
}

fn connect(view: &Retained<WinitView>, keys: &Rc<RefCell<Keys>>) {
    let Some(input) = (unsafe { GCKeyboard::coalescedKeyboard() })
        .and_then(|keyboard| unsafe { keyboard.keyboardInput() })
    else {
        return;
    };
    let (view, keys) = (view.clone(), keys.clone());
    let handler = RcBlock::new(
        move |_input: NonNull<GCKeyboardInput>,
              _key: NonNull<GCControllerButtonInput>,
              usage: isize,
              pressed: Bool| {
            key_changed(&view, &keys, usage, pressed.as_bool());
        },
    );
    // GameController copies the block, so this one may go.
    unsafe { input.setKeyChangedHandler(&*handler as *const _ as *mut _) };
}

fn key_changed(view: &WinitView, keys: &RefCell<Keys>, usage: isize, pressed: bool) {
    // GameController calls the handlers on the main queue unless told otherwise; the app's
    // events are the main thread's, so a key from anywhere else is dropped, not mishandled.
    if MainThreadMarker::new().is_none() {
        tracing::warn!("a hardware key arrived off the main thread and was dropped");
        return;
    }
    let Some(code) = u32::try_from(usage).ok().and_then(hid::key_code) else {
        return;
    };
    let (modifiers_changed, shift) = {
        let mut keys = keys.borrow_mut();
        if pressed {
            // A key already down is not pressed again, and while the view takes text the keys
            // that write it are the text system's.
            if keys.down.contains(&usage)
                || (hid::is_text_key(code) && unsafe { view.isFirstResponder() })
            {
                return;
            }
            keys.down.insert(usage);
        } else if !keys.down.remove(&usage) {
            // Its press was not passed on, so its release is not either.
            return;
        }
        let now = keys.modifiers_down();
        let changed = (now != keys.modifiers).then_some(now);
        keys.modifiers = now;
        (changed, now.contains(ModifiersState::SHIFT))
    };
    let state = if pressed { ElementState::Pressed } else { ElementState::Released };
    if let Some(state) = modifiers_changed {
        view.send_window_event(WindowEvent::ModifiersChanged(Modifiers::from(state)));
    }
    view.send_window_event(WindowEvent::KeyboardInput {
        device_id: DEVICE_ID,
        event: KeyEvent {
            physical_key: PhysicalKey::Code(code),
            logical_key: hid::logical_key(code, shift),
            text: None,
            location: hid::location(code),
            state,
            repeat: false,
            platform_specific: KeyEventExtra {},
        },
        is_synthetic: false,
    });
}

/// Lets go of every key the app was told is down.
fn release_all(view: &WinitView, keys: &RefCell<Keys>) {
    let down: Vec<isize> = keys.borrow().down.iter().copied().collect();
    for usage in down {
        key_changed(view, keys, usage, false);
    }
}
