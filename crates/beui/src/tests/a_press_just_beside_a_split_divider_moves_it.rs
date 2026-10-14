use super::*;
use crate::input::CursorIcon;
use crate::unstyled::SPLITTER_THICKNESS;

#[test]
fn a_press_just_beside_a_split_divider_moves_it() {
    let _root = crate::reactive::RootScope::new();
    let SplitDock {
        document, presses, ..
    } = split_dock(None);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let left = harness.rect(harness.find("content.1"));
    let right = harness.rect(harness.find("content.2"));
    let middle = (left.right() + right.left()) / 2.0;
    let edge = middle + SPLITTER_THICKNESS / 2.0;

    let beside = pos2(edge + 3.0, right.center().y);
    let output = harness.frame(vec![Event::PointerMoved(beside)]);
    assert_eq!(output.window.cursor, CursorIcon::ResizeHorizontal);

    harness.drag(beside, beside + vec2(50.0, 0.0));
    harness.frame(Vec::new());
    let moved = harness.rect(harness.find("content.1"));
    assert!(
        (moved.width() - left.width() - 50.0).abs() < 1.0,
        "the divider follows a press just beside it, from {} to {} wide",
        left.width(),
        moved.width()
    );
    assert_eq!(
        presses.get_untracked(),
        0,
        "the pane beside the divider never heard the press"
    );

    let right = harness.rect(harness.find("content.2"));
    let far = pos2(edge + 50.0 + 12.0, right.center().y);
    let output = harness.frame(vec![Event::PointerMoved(far)]);
    assert_ne!(output.window.cursor, CursorIcon::ResizeHorizontal);
    harness.drag(far, far + vec2(50.0, 0.0));
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(harness.find("content.1")).width(),
        moved.width(),
        "a press farther out leaves the divider alone"
    );
    assert_eq!(
        presses.get_untracked(),
        1,
        "the pane hears a press farther out"
    );
}
