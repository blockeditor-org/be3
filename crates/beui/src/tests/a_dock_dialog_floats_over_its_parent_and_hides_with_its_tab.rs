use super::*;

#[test]
fn a_dock_dialog_floats_over_its_parent_and_hides_with_its_tab() {
    let _root = crate::reactive::RootScope::new();
    let dock = dialog_dock(vec![1, 2], vec![(10, 1, false)]);
    let layout = dock.layout;
    let mut harness = Harness::sized(dock.document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let parent = harness.rect(harness.find("content.1"));
    let dialog = harness.rect(harness.find("dock.dialog.10"));
    assert_eq!(
        dialog.size(),
        dialog_size(10),
        "the dialog is the size it asked for"
    );
    assert!(
        (dialog.center().x - parent.center().x).abs() < 1.0,
        "the dialog is centred over its parent: {dialog:?} over {parent:?}"
    );
    assert!(
        parent.contains_rect(dialog),
        "the dialog sits inside its parent: {dialog:?} in {parent:?}"
    );
    assert_eq!(
        harness.document().focused_node(),
        Some(harness.find("dialog.10.field")),
        "the dialog takes the focus when it opens"
    );

    layout.show(&2);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        harness.document().find_test_id("dock.dialog.10").is_none(),
        "the dialog hides with its parent's tab"
    );

    layout.show(&1);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(harness.find("dock.dialog.10")),
        dialog,
        "the dialog comes back where it was when its parent's tab is shown again"
    );
}
