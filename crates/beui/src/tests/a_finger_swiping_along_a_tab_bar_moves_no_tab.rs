use super::*;
use crate::unstyled::dock_state;

#[test]
fn a_finger_swiping_along_a_tab_bar_moves_no_tab() {
    let (document, dock) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let before = dock_state(harness.document(), dock);
    let first = harness.center(dock_tab(harness.document(), dock, "Tab 1"));
    let second = harness.center(dock_tab(harness.document(), dock, "Tab 2"));

    harness.finger_drag(&[first, first + vec2(6.0, 0.0), second]);
    harness.frame(Vec::new());

    assert_eq!(
        dock_state(harness.document(), dock),
        before,
        "a finger sliding along the bar scrolls it rather than picking up the tab it started on"
    );
}
