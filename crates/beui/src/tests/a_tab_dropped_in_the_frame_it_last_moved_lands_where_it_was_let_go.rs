use super::*;
use crate::dock::unstyled::{DockApi, Entry, TabId};

#[test]
fn a_tab_dropped_in_the_frame_it_last_moved_lands_where_it_was_let_go() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let first = harness.center(dock_tab(harness.document(), dock, "Tab 1"));
    let second = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    let past = pos2(second.right() + 4.0, second.center().y);
    let button = |pos, pressed| Input::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
    };

    harness.frame(vec![Input::PointerMoved(first)]);
    harness.frame(vec![button(first, true)]);
    harness.frame(vec![Input::PointerMoved(first + (past - first) * 0.5)]);
    harness.frame(vec![Input::PointerMoved(past), button(past, false)]);
    harness.frame(Vec::new());

    let state = harness.document().api::<DockApi>(dock).state();
    let leaf = state.leaves(state.main())[0];
    assert_eq!(
        state.entries(leaf),
        vec![Entry::Tab(TabId::new(2)), Entry::Tab(TabId::new(1))],
        "the drop lands where the pointer was let go, not where it was a frame before"
    );
}
