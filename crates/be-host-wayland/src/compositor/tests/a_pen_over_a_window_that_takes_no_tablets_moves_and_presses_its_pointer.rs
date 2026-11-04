use super::*;
use beui::{Pen, PenPhase};

#[test]
fn a_pen_over_a_window_that_takes_no_tablets_moves_and_presses_its_pointer() {
    let mut harness = Harness::new();
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    let at = harness.window(id).center();
    let pen = |phase, pressure| Input::Pen {
        phase,
        pos: at,
        pen: Pen {
            pressure,
            ..Pen::default()
        },
    };

    harness.frame(vec![pen(PenPhase::Move, 0.0)]);
    harness.frame(vec![pen(PenPhase::Down(PointerButton::Primary), 0.5)]);
    harness.frame(vec![pen(PenPhase::Up(PointerButton::Primary), 0.0)]);
    harness.settle();

    assert!(
        harness.client.received.pointer_entered.is_some(),
        "the pen moves the window's pointer onto it"
    );
    assert_eq!(
        harness.client.received.buttons,
        vec![(0x110, true), (0x110, false)],
        "the tip touching and lifting is a left click"
    );
}
