use super::*;
use crate::unstyled::dock_state;

#[test]
fn a_modifier_drag_on_a_docked_pane_moves_nothing_and_reaches_nothing() {
    let SplitDock {
        document,
        dock,
        presses,
    } = split_dock(Some(Modifiers::LOGO));
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let before = dock_state(harness.document(), dock);
    let content = harness.center(harness.find("content.1"));
    let right = harness.center(harness.find("content.2"));
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 1"));

    drag_with(&mut harness, content, right, Modifiers::LOGO);
    drag_with(&mut harness, tab, right, Modifiers::LOGO);
    harness.frame(Vec::new());

    assert_eq!(
        dock_state(harness.document(), dock),
        before,
        "neither the content nor the tab carried anything anywhere"
    );
    assert_eq!(
        presses.get(),
        0,
        "the presses made with the modifier never reached the content"
    );
}
