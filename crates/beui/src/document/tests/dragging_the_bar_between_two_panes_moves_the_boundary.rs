use super::*;
use crate::unstyled::dock_state;

#[test]
fn dragging_the_bar_between_two_panes_moves_the_boundary() {
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 2"));
    harness.drag(tab, pos2(WIDE_VIEWPORT.x - 20.0, WIDE_VIEWPORT.y / 2.0));
    harness.frame(Vec::new());
    let before = harness.rect(harness.find("content.1"));

    let middle = pos2(before.right() + 3.0, WIDE_VIEWPORT.y / 2.0);
    harness.drag(middle, pos2(middle.x - 200.0, middle.y));
    harness.frame(Vec::new());

    let after = harness.rect(harness.find("content.1"));
    assert!(
        after.width() < before.width() - 150.0,
        "dragging the bar left gives the pane on the left less room, \
         {} points instead of {}",
        after.width(),
        before.width()
    );
    let state = dock_state(harness.document(), dock);
    assert_eq!(
        state.leaves(state.main()).len(),
        2,
        "moving the bar leaves both panes in place"
    );
}
