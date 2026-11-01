use beui::input::{Pen, PenPhase, PointerButton, TouchId, TouchPhase};
use beui::input::{PointerId, PointerPhase};
use block_editor_plugin::TimedInput;

use super::*;

fn watching(
    events: Rc<std::cell::RefCell<Vec<Event>>>,
    phases: Rc<std::cell::RefCell<Vec<(PointerId, PointerPhase)>>>,
) -> Windowed {
    Windowed::empty().hooked(move |context| {
        context.input(|input| {
            events.borrow_mut().extend(input.events.iter().cloned());
            phases.borrow_mut().extend(
                input
                    .pointer_events
                    .iter()
                    .map(|event| (event.pointer, event.phase)),
            );
        });
    })
}

#[test]
fn a_view_that_loses_the_keyboard_mid_press_keeps_the_press() {
    let events = Rc::new(std::cell::RefCell::new(Vec::new()));
    let phases = Rc::new(std::cell::RefCell::new(Vec::new()));
    let shown = whole(80.0, 60.0);
    let mut surface = launched(
        &EditorHost::default(),
        watching(events.clone(), phases.clone()),
    );
    one(&mut surface, &shown);
    let finger = TouchId {
        device: 1,
        finger: 2,
    };
    let pen = |phase: PenPhase, pos| Input::Pen {
        phase,
        pos,
        pen: Pen {
            pressure: 0.5,
            ..Pen::default()
        },
    };
    let mut send = |input: Input| surface.input(&shown, &TimedInput::from(input), None);
    send(Input::Focus(true));
    send(Input::PointerMoved(pos2(10.0, 10.0)));
    send(Input::PointerButton {
        pos: pos2(10.0, 10.0),
        button: PointerButton::Primary,
        pressed: true,
    });
    send(Input::Touch {
        id: finger,
        phase: TouchPhase::Start,
        pos: pos2(50.0, 10.0),
        force: None,
    });
    send(pen(
        PenPhase::Down(PointerButton::Primary),
        pos2(30.0, 40.0),
    ));
    one(&mut surface, &shown);

    let mut send = |input: Input| surface.input(&shown, &TimedInput::from(input), None);
    send(Input::Focus(false));
    send(Input::PointerMoved(pos2(40.0, 30.0)));
    send(Input::Touch {
        id: finger,
        phase: TouchPhase::Move,
        pos: pos2(55.0, 15.0),
        force: None,
    });
    send(pen(PenPhase::Move, pos2(35.0, 45.0)));
    one(&mut surface, &shown);
    assert!(
        events.borrow().contains(&Event::Focus(false)),
        "the lost keyboard is reported at once"
    );
    assert!(
        !phases
            .borrow()
            .iter()
            .any(|(_, phase)| matches!(phase, PointerPhase::Gone | PointerPhase::Cancel)),
        "the presses the host still forwards go on: {:?}",
        phases.borrow()
    );

    let mut send = |input: Input| surface.input(&shown, &TimedInput::from(input), None);
    send(Input::PointerButton {
        pos: pos2(40.0, 30.0),
        button: PointerButton::Primary,
        pressed: false,
    });
    send(Input::Touch {
        id: finger,
        phase: TouchPhase::End,
        pos: pos2(55.0, 15.0),
        force: None,
    });
    send(pen(PenPhase::Up(PointerButton::Primary), pos2(35.0, 45.0)));
    one(&mut surface, &shown);
    let phases = phases.borrow();
    for pointer in [PointerId::Mouse, PointerId::Touch(finger), PointerId::Pen] {
        assert!(
            phases.contains(&(pointer, PointerPhase::Up(PointerButton::Primary))),
            "{pointer:?} is let go where the host says: {phases:?}"
        );
    }
}
