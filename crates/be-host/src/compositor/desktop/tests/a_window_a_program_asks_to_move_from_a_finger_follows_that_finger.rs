use super::*;

#[test]
fn a_window_a_program_asks_to_move_from_a_finger_follows_that_finger() {
    let _root = beui::reactive::RootScope::new();
    let mut carrying = Carrying::open(pos2(100.0, 100.0), vec2(300.0, 200.0));
    let start = carrying.placed();
    let finger = PointerId::Touch(beui::TouchId {
        device: 0,
        finger: 0,
    });

    let held = start.min + HELD;
    carrying.pointer_by(finger, PointerPhase::Down(PointerButton::Primary), held);
    assert!(
        carrying.inside(|key| begin(key, Kind::Move)),
        "the program's window is carried by the window it is shown in"
    );
    carrying.stepped.advance(STEP);
    carrying.pointer_by(finger, PointerPhase::Move, held + vec2(60.0, 40.0));
    let moved = carrying.placed();
    assert_eq!(
        moved,
        start.translate(vec2(60.0, 40.0)),
        "the window follows the finger from where the program was touched"
    );

    carrying.drag_to(held + vec2(-80.0, 200.0));
    assert_eq!(
        carrying.placed(),
        moved,
        "the mouse moving meanwhile does not carry the window"
    );

    carrying.pointer_by(
        finger,
        PointerPhase::Up(PointerButton::Primary),
        held + vec2(60.0, 40.0),
    );
    carrying.drag_to(held);
    assert_eq!(
        carrying.placed(),
        moved,
        "the window is let go with the finger"
    );
}
