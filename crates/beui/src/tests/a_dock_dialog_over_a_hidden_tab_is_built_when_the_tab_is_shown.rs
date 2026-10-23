use super::*;

#[test]
fn a_dock_dialog_over_a_hidden_tab_is_built_when_the_tab_is_shown() {
    let _root = crate::reactive::RootScope::new();
    let dock = dialog_dock(vec![1, 2], vec![(10, 2, false)]);
    let layout = dock.layout;
    let mut harness = Harness::sized(dock.document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        harness.document().find_test_id("dock.dialog.10").is_none(),
        "a dialog over a hidden tab is not shown"
    );

    layout.show(&2);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let parent = harness.rect(harness.find("content.2"));
    let dialog = harness.rect(harness.find("dock.dialog.10"));
    assert!(
        parent.contains_rect(dialog),
        "the dialog opens over its parent once that is shown: {dialog:?} in {parent:?}"
    );
    assert_eq!(
        harness.document().focused_node(),
        Some(harness.find("dialog.10.field")),
        "the dialog takes the focus when it first shows"
    );
}
