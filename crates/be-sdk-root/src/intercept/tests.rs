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

#[test]
fn the_session_hears_only_the_modifiers_let_go_after_its_chord() {
    let mut keys = intercepting();
    let screen = Some(Handed::Screen(SESSION, SESSION_SURFACE));
    // Ctrl, then Shift, held before Tab: the router hands the modifiers to the focus.
    for held in [CTRL, CTRL_SHIFT] {
        let heard = keys.hand(None, ROOT, screen, Vec::new(), Some((held, None)));
        assert!(heard.is_empty(), "no chord is held yet: {heard:?}");
    }
    let heard = keys.hand(
        None,
        ROOT,
        screen,
        vec![to(ROOT, Surface::Root(0), tab(true), CTRL_SHIFT)],
        None,
    );
    let inputs: Vec<&Input> = heard.iter().map(|delivery| &delivery.input).collect();
    assert_eq!(
        inputs,
        [
            &Input::Modifiers(CTRL_SHIFT),
            &Input::InterceptedKey {
                key: Key::Tab,
                pressed: true,
                repeat: false,
            },
        ],
        "the session hears the chord with the modifiers held as it was pressed"
    );
    assert!(heard.iter().all(|delivery| delivery.target == SESSION));
    // Shift let go after the press: the session hears it, and Ctrl still held.
    let heard = keys.hand(None, ROOT, screen, Vec::new(), Some((CTRL, None)));
    let inputs: Vec<&Input> = heard.iter().map(|delivery| &delivery.input).collect();
    assert_eq!(inputs, [&Input::Modifiers(CTRL)]);
    let heard = keys.hand(None, ROOT, screen, Vec::new(), Some((Modifiers::NONE, None)));
    let inputs: Vec<&Input> = heard.iter().map(|delivery| &delivery.input).collect();
    assert_eq!(inputs, [&Input::Modifiers(Modifiers::NONE)]);
    let heard = keys.hand(None, ROOT, screen, Vec::new(), Some((CTRL, None)));
    assert!(heard.is_empty(), "nothing is held of the chord any more: {heard:?}");
}

#[test]
fn a_chord_handed_to_the_focus_keeps_its_place_among_what_follows() {
    let mut keys = intercepting();
    // The palette, a surface of the root's own, has the keyboard: the chord goes to it.
    let focus = Some(Handed::Focus(PALETTE, PALETTE_SURFACE));
    let x = Input::Text("x".to_owned());
    let heard = keys.hand(
        None,
        ROOT,
        focus,
        vec![
            to(ROOT, Surface::Root(0), tab(true), CTRL),
            to(PALETTE, PALETTE_SURFACE, x.clone(), CTRL),
        ],
        None,
    );
    let heard: Vec<(LayerId, &Input)> = heard
        .iter()
        .map(|delivery| (delivery.target, &delivery.input))
        .collect();
    assert_eq!(heard, [(PALETTE, &tab(true)), (PALETTE, &x)]);
    // Its release follows it there.
    let heard = keys.hand(
        None,
        ROOT,
        Some(Handed::Screen(SESSION, SESSION_SURFACE)),
        vec![to(ROOT, Surface::Root(0), tab(false), CTRL)],
        None,
    );
    assert_eq!(heard.len(), 1);
    assert_eq!((heard[0].target, &heard[0].input), (PALETTE, &tab(false)));
}
