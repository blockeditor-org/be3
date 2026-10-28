use super::*;
use crate::dock::unstyled::{DockApi, Entry, TabId};

#[test]
fn dragging_a_tab_along_its_bar_parts_the_tabs_it_passes() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let second = dock_tab_extent(&harness, dock, "Tab 2", 2);
    let third = dock_tab_extent(&harness, dock, "Tab 3", 3);
    let from = harness.center(dock_tab(harness.document(), dock, "Tab 3"));
    let over = pos2(
        from.x - (third.center().x - second.center().x) - 10.0,
        from.y,
    );

    harness.press_at(from);
    harness.frame(vec![Input::PointerMoved(over)]);
    harness.frames_to_settle();

    let parted = dock_tab_extent(&harness, dock, "Tab 2", 2);
    assert!(
        (parted.left() - third.left()).abs() < 1.0,
        "the tab passed glides aside into the place the carried one left, at {}, not {}",
        third.left(),
        parted.left()
    );
    let carried = dock_tab_extent(&harness, dock, "Tab 3", 3);
    assert!(
        (carried.top() - third.top()).abs() < 1.0,
        "the carried tab stays in its bar, following the pointer only along it"
    );
    let state = harness.document().api::<DockApi>(dock).state();
    let leaf = state.leaves(state.main())[0];
    let tabs = |ids: [u64; 3]| ids.map(|id| Entry::Tab(TabId::new(id))).to_vec();
    assert_eq!(
        state.entries(leaf),
        tabs([1, 2, 3]),
        "nothing moves in the dock while the tab is carried"
    );

    harness.release_at(over);
    harness.frames_to_settle();

    let state = harness.document().api::<DockApi>(dock).state();
    assert_eq!(
        state.entries(leaf),
        tabs([1, 3, 2]),
        "the dropped tab takes the place it was shown in"
    );
    let landed = dock_tab_extent(&harness, dock, "Tab 3", 3);
    assert!(
        (landed.left() - second.left()).abs() < 1.0,
        "the dropped tab settles into its new place"
    );
}
