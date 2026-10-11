use super::*;
use crate::unstyled::{TabId, dock_state};

#[test]
fn a_dock_dialog_is_never_a_tab() {
    let dock = dialog_dock(vec![1, 2, 10], vec![(10, 1, false)]);
    let node = dock.dock;
    let mut harness = Harness::sized(dock.document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let tabs = || dock_state(harness.document(), node).all_tabs();
    assert_eq!(
        tabs(),
        vec![TabId::new(1), TabId::new(2)],
        "a key declared as a dialog is not docked, even when it is declared as a tab too"
    );
    let before = dock_state(harness.document(), node);

    let dialog = harness.rect(harness.find("dock.dialog.10"));
    let title = pos2(dialog.center().x, dialog.top() - 10.0);
    let pane = harness.rect(harness.find("content.1"));
    drag_with(
        &mut harness,
        title,
        pos2(pane.right() - 20.0, pane.center().y),
        Modifiers::NONE,
    );
    drag_with(
        &mut harness,
        title,
        pos2(pane.right() - 20.0, pane.center().y),
        Modifiers::ALT,
    );
    harness.frame(Vec::new());

    assert_eq!(
        dock_state(harness.document(), node),
        before,
        "dragging the dialog leaves the dock as it was"
    );
    assert_eq!(
        harness.rect(harness.find("dock.dialog.10")),
        dialog,
        "the dialog stays over its parent"
    );
}
