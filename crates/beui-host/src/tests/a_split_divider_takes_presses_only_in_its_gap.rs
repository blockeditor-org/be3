use super::*;
use beui::CursorIcon;

#[test]
fn a_split_divider_takes_presses_only_in_its_gap() {
    let _root = crate::reactive::RootScope::new();
    let SplitDock {
        document, presses, ..
    } = split_dock(None);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let left = harness.rect(harness.find("content.1"));
    let right = harness.rect(harness.find("content.2"));
    let middle = (left.right() + right.left()) / 2.0;

    let gap = pos2(middle, right.center().y);
    let output = harness.frame(vec![Input::PointerMoved(gap)]);
    assert_eq!(
        output.main().window.requests.cursor,
        CursorIcon::ResizeHorizontal
    );
    harness.drag(gap, gap + vec2(50.0, 0.0));
    harness.frame(Vec::new());
    let moved = harness.rect(harness.find("content.1"));
    assert!(
        (moved.width() - left.width() - 50.0).abs() < 1.0,
        "the divider follows a press in the gap, from {} to {} wide",
        left.width(),
        moved.width()
    );
    assert_eq!(
        presses.get(),
        0,
        "the pane beside the gap never heard the press"
    );

    let right = harness.rect(harness.find("content.2"));
    let inside = pos2(right.left() + 4.0, right.center().y);
    let output = harness.frame(vec![Input::PointerMoved(inside)]);
    assert_ne!(
        output.main().window.requests.cursor,
        CursorIcon::ResizeHorizontal,
        "past the few points either side of the line that grab it, the pane's content is the pane's"
    );
    harness.drag(inside, inside + vec2(50.0, 0.0));
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(harness.find("content.1")).width(),
        moved.width(),
        "a press on the pane's content leaves the divider alone"
    );
    assert_eq!(
        presses.get(),
        1,
        "the pane hears a press just past the divider's reach"
    );
}
