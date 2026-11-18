use super::*;

#[test]
fn escape_puts_a_dragged_splitter_back_where_it_was_pressed() {
    let _root = crate::reactive::RootScope::new();
    let SplitDock { document, .. } = split_dock(None);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let left = harness.rect(harness.find("content.1"));
    let right = harness.rect(harness.find("content.2"));
    let gap = pos2((left.right() + right.left()) / 2.0, right.center().y);

    harness.down(gap, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(gap + vec2(2.0, 0.0))]);
    harness.frame(Vec::new());
    assert!(
        (harness.rect(harness.find("content.1")).width() - left.width() - 2.0).abs() < 1.0,
        "the splitter follows the mouse from its first point, short of the drag threshold"
    );
    harness.frame(vec![Input::PointerMoved(gap + vec2(50.0, 0.0))]);
    harness.frame(Vec::new());
    assert!(
        (harness.rect(harness.find("content.1")).width() - left.width() - 50.0).abs() < 1.0,
        "and on as it moves"
    );
    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.up(gap + vec2(50.0, 0.0), PointerButton::Primary);
    harness.settle();
    assert_eq!(
        harness.rect(harness.find("content.1")).width(),
        left.width(),
        "Escape puts it back to where it was pressed, and the release keeps it there"
    );
}
