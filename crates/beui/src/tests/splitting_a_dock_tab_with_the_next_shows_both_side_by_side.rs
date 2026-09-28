use super::*;
use crate::unstyled::dock_state;

#[test]
fn splitting_a_dock_tab_with_the_next_shows_both_side_by_side() {
    let (document, dock) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let pos = harness.center(dock_tab(harness.document(), dock, "Tab 2"));

    harness.frame(vec![Event::PointerMoved(pos)]);
    harness.frame(vec![Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(Vec::new());
    let row = text_within(harness.document(), dock, "Split with next tab")
        .expect("the menu offers to split the tab with the next");
    harness.click(harness.center(row));
    harness.frame(Vec::new());

    let state = dock_state(harness.document(), dock);
    let leaf = state.leaves(state.main())[0];
    assert_eq!(
        state.entries(leaf).len(),
        2,
        "the two tabs become one group in the bar"
    );
    let left = harness.rect(harness.find("content.2"));
    let right = harness.rect(harness.find("content.3"));
    assert!(
        right.left() > left.right() && (right.top() - left.top()).abs() < 1.0,
        "both tabs of the group are shown at once, side by side: {left:?} and {right:?}"
    );
}
