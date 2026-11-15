use super::*;
use beui_view::types::TouchId;
use beui_view::types::input::PointerId;

/// A host's drag begins at the last pointer point, named the mouse whatever pressed; on a
/// touch screen the finger moves it, and once it has, another pointer does not.
#[test]
fn a_drag_begun_at_a_mouse_point_follows_the_first_pointer_that_moves_it() {
    let fixture = Fixture::new();
    fixture
        .target("list", rect(0.0, 0.0, 100.0, 100.0), 1)
        .register();
    fixture.begin(pos2(150.0, 50.0));
    fixture.log.take();
    let finger = PointerId::Touch(TouchId {
        device: 0,
        finger: 1,
    });
    let order = |_| (Vec::new(), WindowId::MAIN);
    Driver::track(&fixture.board, at_point(pos2(50.0, 50.0)).by(finger), &order);
    assert!(
        fixture.log.take().contains(&"list over 50 50".to_owned()),
        "the finger moves a drag begun at a mouse point"
    );
    Driver::track(&fixture.board, at_point(pos2(150.0, 50.0)), &order);
    assert!(
        !fixture.log.take().iter().any(|line| line == "list left"),
        "and the mouse no longer does"
    );
}
