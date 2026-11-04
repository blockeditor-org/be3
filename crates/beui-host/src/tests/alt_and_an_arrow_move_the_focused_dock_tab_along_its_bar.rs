use super::*;
use crate::dock::unstyled::{DockApi, Entry, TabId};

#[test]
fn alt_and_an_arrow_move_the_focused_dock_tab_along_its_bar() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let first = dock_tab_extent(&harness, dock, "Tab 1", 1);
    harness.click(harness.center(dock_tab(harness.document(), dock, "Tab 1")));
    harness.frame(Vec::new());

    harness.key(Key::ArrowRight, Modifiers::ALT);
    harness.frames_to_settle();

    let state = harness.document().api::<DockApi>(dock).state();
    let leaf = state.leaves(state.main())[0];
    assert_eq!(
        state.entries(leaf),
        [2, 1, 3].map(|id| Entry::Tab(TabId::new(id))).to_vec(),
        "alt and the right arrow move the focused tab one place along"
    );
    assert_eq!(
        dock_tab_extent(&harness, dock, "Tab 2", 2).left(),
        first.left(),
        "the tab it passed glides into its old place"
    );

    harness.key(Key::ArrowLeft, Modifiers::ALT);
    harness.frames_to_settle();
    let state = harness.document().api::<DockApi>(dock).state();
    assert_eq!(
        state.entries(leaf),
        [1, 2, 3].map(|id| Entry::Tab(TabId::new(id))).to_vec(),
        "alt and the left arrow move it back, keeping the focus on it"
    );
}
