use super::*;
use crate::input::CursorIcon;
use crate::dock::unstyled::DockApi;

#[test]
fn a_floating_windows_grips_lie_outside_it() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    let window = floated_window(&mut harness, dock);
    let bounds = harness.rect(dock);
    let placed = |harness: &Harness| {
        harness
            .document()
            .api::<DockApi>(dock)
            .state()
            .window_rect(window)
            .expect("the window is open")
    };
    let before = placed(&harness);
    let corner = bounds.min + before.max.to_vec2();

    let inside = corner - vec2(2.0, 2.0);
    let output = harness.frame(vec![Input::PointerMoved(inside)]);
    assert_ne!(
        output.main().window.requests.cursor,
        CursorIcon::ResizeNwSe,
        "the window's own content reaches right to its corner"
    );
    harness.drag(inside, inside + vec2(40.0, 30.0));
    harness.frame(Vec::new());
    assert_eq!(
        placed(&harness),
        before,
        "a press inside the corner resizes nothing"
    );

    let outside = corner + vec2(3.0, 3.0);
    let output = harness.frame(vec![Input::PointerMoved(outside)]);
    assert_eq!(output.main().window.requests.cursor, CursorIcon::ResizeNwSe);
    harness.drag(outside, outside + vec2(40.0, 30.0));
    harness.frame(Vec::new());
    assert_eq!(
        placed(&harness),
        Rect::from_min_max(before.min, before.max + vec2(40.0, 30.0)),
        "the grip just outside the corner resizes the window"
    );

    let corner = bounds.min + placed(&harness).max.to_vec2();
    let along = pos2(corner.x - 12.0, corner.y + 3.0);
    let output = harness.frame(vec![Input::PointerMoved(along)]);
    assert_eq!(
        output.main().window.requests.cursor,
        CursorIcon::ResizeNwSe,
        "the corner's grip runs a little way along the edge"
    );
}
