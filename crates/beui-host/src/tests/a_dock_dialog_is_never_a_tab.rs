use super::*;
use crate::dock::unstyled::{DockApi, TabId};

#[test]
fn a_dock_dialog_is_never_a_tab() {
    let _root = crate::reactive::RootScope::new();
    let dock = dialog_dock(vec![1, 2, 10], vec![(10, 1, false)]);
    let node = dock.dock;
    let mut harness = Harness::sized(dock.document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let tabs = || harness.document().api::<DockApi>(node).state().all_tabs();
    assert_eq!(
        tabs(),
        vec![TabId::new(1), TabId::new(2)],
        "a key declared as a dialog is not docked, even when it is declared as a tab too"
    );
    let before = harness.document().api::<DockApi>(node).state();

    let dialog = harness.rect(harness.find("dock.dialog.10"));
    let title = harness.center(harness.find("dock.dialog.10.header"));
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
        harness.document().api::<DockApi>(node).state(),
        before,
        "dragging the dialog leaves the dock as it was"
    );
    let moved = harness.rect(harness.find("dock.dialog.10"));
    assert!(
        moved.center().x > dialog.center().x && moved.center().y > dialog.center().y,
        "dragging its header moves the dialog rather than docking it: {moved:?} from {dialog:?}"
    );
}
