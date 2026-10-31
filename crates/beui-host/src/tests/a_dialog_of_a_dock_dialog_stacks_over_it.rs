use super::*;

#[test]
fn a_dialog_of_a_dock_dialog_stacks_over_it() {
    let _root = crate::reactive::RootScope::new();
    let dock = dialog_dock(vec![1, 2], vec![(11, 10, true), (10, 1, false)]);
    let (layout, pressed) = (dock.layout, dock.pressed);
    let mut harness = Harness::sized(dock.document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let outer = harness.rect(harness.find("dock.dialog.10"));
    let inner = harness.rect(harness.find("dock.dialog.11"));
    let header = harness.rect(harness.find("dock.dialog.10.header"));
    assert!(
        inner.top() > header.bottom() && (inner.center().x - header.center().x).abs() < 1.0,
        "the nested dialog hangs below the header of the dialog it belongs to, pointing at it: {inner:?} below {header:?}"
    );
    assert_eq!(
        harness.document().focused_node(),
        Some(harness.find("dialog.11.field")),
        "the focus goes to the innermost modal dialog"
    );

    harness.click(inner.center());
    assert_eq!(
        pressed.get_untracked(),
        vec![11],
        "the nested dialog is drawn above its parent"
    );
    harness.click(outer.min + vec2(8.0, 8.0));
    assert_eq!(
        pressed.get_untracked(),
        vec![11],
        "the dialog under a modal one hears no press"
    );

    layout.show(&2);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let document = harness.document();
    assert!(
        document.find_test_id("dock.dialog.10").is_none()
            && document.find_test_id("dock.dialog.11").is_none(),
        "both hide with the tab they hang from"
    );

    layout.show(&1);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    harness.click(inner.center());
    assert_eq!(
        pressed.get_untracked(),
        vec![11, 11],
        "shown again, the nested dialog is still on top"
    );
}
