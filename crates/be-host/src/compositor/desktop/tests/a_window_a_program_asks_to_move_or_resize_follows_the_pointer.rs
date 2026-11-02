use super::*;

#[test]
fn a_window_a_program_asks_to_move_or_resize_follows_the_pointer() {
    let _root = beui::reactive::RootScope::new();
    let mut carrying = Carrying::open(pos2(100.0, 100.0), vec2(300.0, 200.0));
    let start = carrying.placed();

    let held = start.min + HELD;
    carrying.pointer(PointerPhase::Down(PointerButton::Primary), held);
    assert!(
        carrying.inside(|key| begin(key, Kind::Move)),
        "the program's window is carried by the window it is shown in"
    );
    carrying.drag_to(held + vec2(60.0, 40.0));
    carrying.release(held + vec2(60.0, 40.0));
    let moved = carrying.placed();
    assert_eq!(
        moved,
        start.translate(vec2(60.0, 40.0)),
        "the window follows the pointer from where the program was pressed"
    );

    let edge = pos2(moved.right() - 2.0, moved.center().y);
    carrying.pointer(PointerPhase::Down(PointerButton::Primary), edge);
    carrying.inside(|key| begin(key, Kind::Resize(be_sdk_api::WindowEdge::Right)));
    carrying.drag_to(edge + vec2(-50.0, 30.0));
    carrying.release(edge + vec2(-50.0, 30.0));
    let resized = carrying.placed();
    assert_eq!(
        (resized.min, resized.size()),
        (moved.min, moved.size() - vec2(50.0, 0.0)),
        "a resize from the right edge moves only that edge"
    );

    carrying.drag_to(edge);
    assert_eq!(carrying.placed(), resized, "the window is let go with the button");
}
