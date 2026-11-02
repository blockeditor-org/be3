use super::*;
use std::{cell::RefCell, rc::Rc};

fn mouse(phase: PointerPhase, x: f32, y: f32) -> Forwarded {
    Forwarded::Pointer {
        event: PointerEvent {
            pointer: PointerId::Mouse,
            window: beui::WindowId::MAIN,
            phase,
            pos: pos2(x, y),
            modifiers: Default::default(),
            clicks: 1,
            force: None,
            pen: None,
            predicted: None,
            time: std::time::Instant::now(),
            source: 0,
        },
        over: None,
    }
}

fn input(events: Vec<Forwarded>) -> ForwardedInput {
    ForwardedInput {
        events,
        rect: Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(400.0, 300.0)),
        hovered: true,
        focused: true,
        pointer: None,
        modifiers: Default::default(),
    }
}

#[test]
fn a_pan_drag_asked_for_late_pans_from_the_press() {
    let _root = beui::reactive::RootScope::new();
    let panned = Rc::new(RefCell::new(Vec2::ZERO));
    let pan = Callback::new({
        let panned = panned.clone();
        move |delta: Vec2| *panned.borrow_mut() += delta
    });
    let mut steering = Steering::IDLE;
    steering.steer(
        input(vec![mouse(
            PointerPhase::Down(PointerButton::Primary),
            100.0,
            100.0,
        )]),
        pan,
    );
    steering.steer(input(vec![mouse(PointerPhase::Move, 115.0, 107.0)]), pan);
    steering.steer(input(vec![mouse(PointerPhase::Move, 130.0, 114.0)]), pan);
    assert!(steering.take_over(pan).is_some());
    let left = steering.steer(input(vec![mouse(PointerPhase::Move, 145.0, 121.0)]), pan);
    assert!(
        left.events.is_empty(),
        "the editor sees no more of the drag"
    );
    assert_eq!(
        *panned.borrow(),
        Vec2::new(45.0, 21.0),
        "the camera moved with the pointer from where it was pressed, moves before the editor asked included"
    );
}
