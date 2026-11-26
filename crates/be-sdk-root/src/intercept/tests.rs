use be_lib_layers::{LayerId, Surface};
use be_lib_ui_types::KeyChord;
use be_lib_ui_types::input::{Input, Key, Keystroke, Modifiers};

use super::{Handed, Intercepting};
use crate::Delivery;

const ROOT: LayerId = LayerId(0);
const SESSION: LayerId = LayerId(7);
const SESSION_SURFACE: Surface = Surface::Screen {
    instance: 3,
    surface: 1,
};
const PALETTE: LayerId = LayerId(9);
const PALETTE_SURFACE: Surface = Surface::Root(5);

const CTRL: Modifiers = Modifiers {
    ctrl: true,
    ..Modifiers::NONE
};
const CTRL_SHIFT: Modifiers = Modifiers {
    ctrl: true,
    shift: true,
    ..Modifiers::NONE
};

fn tab(pressed: bool) -> Input {
    Input::Key(Keystroke {
        key: Some(Key::Tab),
        pressed,
        repeat: false,
        evdev: None,
        text: None,
    })
}

fn to(target: LayerId, surface: Surface, input: Input, modifiers: Modifiers) -> Delivery {
    Delivery {
        target,
        surface,
        input,
        time: None,
        modifiers,
    }
}

/// The session's editor intercepts Ctrl+Tab and Ctrl+Shift+Tab, as the workspace does.
fn intercepting() -> Intercepting {
    Intercepting {
        chords: vec![
            KeyChord::new(Key::Tab, CTRL),
            KeyChord::new(Key::Tab, CTRL_SHIFT),
        ],
        ..Intercepting::default()
    }
}

mod a_chord_handed_to_the_focus_keeps_its_place_among_what_follows;
mod the_session_hears_only_the_modifiers_let_go_after_its_chord;
