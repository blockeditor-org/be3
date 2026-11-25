use be_lib_geometry::{pos2, vec2};

use super::*;
use crate::input::{Key, PointerButton};

#[test]
fn a_catch_turns_what_a_router_delivered_into_what_a_catcher_catches() {
    let mut catch = Catch::default();
    let now = Instant::now();
    let window = crate::window::WindowId::MAIN;
    // A press: where the pointer moved, the keyboard it gave, then the button.
    let caught = catch.caught(
        [
            (Input::PointerMoved(pos2(10.0, 20.0)), now),
            (Input::Focus(true), now),
            (
                Input::PointerButton {
                    pos: pos2(10.0, 20.0),
                    button: PointerButton::Primary,
                    pressed: true,
                },
                now,
            ),
        ],
        window,
        vec2(320.0, 240.0),
    );
    let phases: Vec<_> = caught
        .pointer_events()
        .map(|event| (event.phase, event.pos, event.pointer))
        .collect();
    assert_eq!(
        phases,
        vec![
            (PointerPhase::Move, pos2(10.0, 20.0), PointerId::Mouse),
            (
                PointerPhase::Down(PointerButton::Primary),
                pos2(10.0, 20.0),
                PointerId::Mouse
            ),
        ]
    );
    assert!(caught.events.contains(&Forwarded::Focus(true)));
    // Its points are the surface's: the catcher is the surface, from its origin.
    assert_eq!(caught.rect, Rect::from_min_size(Pos2::ZERO, vec2(320.0, 240.0)));
    assert!(caught.focused && caught.hovered);
    assert_eq!(caught.pointer, Some(pos2(10.0, 20.0)));

    // A key, held with Shift, says the modifiers it was pressed with; a wheel turning is where
    // the pointer was last; the pointer leaving is `Left`.
    let caught = catch.caught(
        [
            (Input::Modifiers(Modifiers::SHIFT), now),
            (
                Input::Key(Keystroke {
                    key: Some(Key::A),
                    pressed: true,
                    repeat: false,
                    evdev: Some(30),
                    text: Some("A".to_owned()),
                }),
                now,
            ),
            (Input::Scroll(ScrollDelta::Lines(vec2(0.0, 1.0))), now),
            (Input::PointerLeft, now),
        ],
        window,
        vec2(320.0, 240.0),
    );
    assert_eq!(
        caught.events[1],
        Forwarded::Input(Event::Key {
            press: KeyPress {
                key: Key::A,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::SHIFT,
            },
            evdev: Some(30),
            text: Some("A".to_owned()),
        })
    );
    let Forwarded::Pointer { event, .. } = &caught.events[2] else {
        panic!("the wheel is a pointer event");
    };
    assert_eq!(event.pos, pos2(10.0, 20.0));
    assert_eq!(
        event.phase,
        PointerPhase::Scroll {
            delta: vec2(0.0, crate::input::SCROLL_LINE),
            notches: Some(vec2(0.0, 1.0)),
        }
    );
    assert_eq!(caught.events[3], Forwarded::Left);
    assert!(!caught.hovered);
}
