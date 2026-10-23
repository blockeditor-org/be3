use super::*;
use crate::test_client::PenSeen;
use beui::{Pen, PenPhase, vec2};

#[test]
fn a_pen_over_a_window_reaches_it_as_a_tablet_tool_with_its_pressure() {
    let mut harness = Harness::new();
    let _tablets = harness.client.tablet_seat();
    let (_window, id) = harness.open();
    let at = harness.window(id).center();
    let pen = |phase, pos, pressure| Input::Pen {
        phase,
        pos,
        pen: Pen {
            pressure,
            tilt: vec2(20.0, -10.0),
            eraser: false,
        },
    };

    harness.frame(vec![pen(PenPhase::Move, at, 0.0)]);
    harness.frame(vec![pen(PenPhase::Down(PointerButton::Primary), at, 0.5)]);
    harness.frame(vec![pen(PenPhase::Move, at + vec2(10.0, 0.0), 1.0)]);
    harness.frame(vec![pen(
        PenPhase::Up(PointerButton::Primary),
        at + vec2(10.0, 0.0),
        0.0,
    )]);
    harness.frame(vec![pen(PenPhase::Gone, at + vec2(10.0, 0.0), 0.0)]);
    harness.settle();

    let seen = &harness.client.received.protocols.pen;
    let contact: Vec<&PenSeen> = seen
        .iter()
        .filter(|seen| {
            matches!(
                seen,
                PenSeen::Eraser(_) | PenSeen::In | PenSeen::Down | PenSeen::Up | PenSeen::Out
            )
        })
        .collect();
    assert_eq!(
        contact,
        [
            &PenSeen::Eraser(false),
            &PenSeen::In,
            &PenSeen::Down,
            &PenSeen::Up,
            &PenSeen::Out
        ],
        "the window hears a pen come into range, touch down, lift and leave"
    );
    assert!(
        seen.contains(&PenSeen::Pressure(65535)) && seen.contains(&PenSeen::Tilt(20.0, -10.0)),
        "the window hears how hard the pen pressed and how it was tilted: {seen:?}"
    );
    assert!(
        harness.client.received.buttons.is_empty(),
        "a window that takes tablets hears nothing of the pen as a pointer"
    );
}
