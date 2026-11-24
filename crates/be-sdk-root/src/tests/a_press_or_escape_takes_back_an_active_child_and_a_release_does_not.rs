use be_lib_geometry::vec2;
use be_lib_ui_types::input::{
    Input, Key, Keystroke, Pen, PenPhase, PointerButton, TouchId, TouchPhase,
};

use super::*;

fn escape(pressed: bool, repeat: bool) -> Input {
    Input::Key(Keystroke {
        key: Some(Key::Escape),
        pressed,
        repeat,
        evdev: None,
        text: None,
    })
}

#[test]
fn a_press_or_escape_takes_back_an_active_child_and_a_release_does_not() {
    let at = pos2(4.0, 4.0);
    let button = |pressed| Input::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
    };
    assert!(presses(&button(true)));
    assert!(!presses(&button(false)));
    let touch = |phase| Input::Touch {
        id: TouchId {
            device: 0,
            finger: 1,
        },
        phase,
        pos: at,
        force: None,
    };
    assert!(presses(&touch(TouchPhase::Start)));
    assert!(!presses(&touch(TouchPhase::Move)));
    assert!(presses(&escape(true, false)));
    assert!(!presses(&escape(true, true)), "a held Escape's repeats take back nothing more");
    assert!(!presses(&escape(false, false)));
    let pen = |phase| Input::Pen {
        phase,
        pos: at,
        pen: Pen {
            pressure: 1.0,
            tilt: vec2(0.0, 0.0),
            eraser: false,
        },
    };
    assert!(presses(&pen(PenPhase::Down(PointerButton::Primary))));
    assert!(!presses(&pen(PenPhase::Move)));
}
