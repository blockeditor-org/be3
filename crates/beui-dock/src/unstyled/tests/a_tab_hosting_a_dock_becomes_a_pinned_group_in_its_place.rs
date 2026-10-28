use super::*;
use crate::unstyled::state::joined_key;

#[test]
fn a_tab_hosting_a_dock_becomes_a_pinned_group_in_its_place() {
    let mut state = DockState::default();
    state.reconcile(&spec(pane("main", &[1, 2, 3])));
    let leaf = leaf_of(&state, 2);
    state.show(TabId::new(2));

    let key = joined_key(TabId::new(2));
    let group = state
        .host_group(
            TabId::new(2),
            &key,
            &pane("joined:2/inner", &[10, 11]),
            None,
        )
        .expect("the hosted dock declares tabs");

    assert_eq!(
        state.entries(leaf),
        vec![
            Entry::Tab(TabId::new(1)),
            Entry::Group(group),
            Entry::Tab(TabId::new(3))
        ],
        "the group takes the place of the tab that hosts it"
    );
    assert!(
        state.is_pinned(group),
        "the hosted dock is locked in its group"
    );
    assert!(!state.contains(TabId::new(2)), "the hosting tab is gone");
    assert_eq!(
        state.home(TabId::new(10)),
        Some(DockHome::Group(group)),
        "the hosted tabs live in the group"
    );
    assert!(
        state
            .focused_tab()
            .is_some_and(|tab| [10, 11].contains(&tab.value())),
        "the focus moves from the hosting tab into the group"
    );

    state.unhost_group(group, TabId::new(2));
    assert_eq!(
        state.entries(leaf),
        vec![
            Entry::Tab(TabId::new(1)),
            Entry::Tab(TabId::new(2)),
            Entry::Tab(TabId::new(3))
        ],
        "ending the hosting puts the tab back where the group was"
    );
    assert!(!state.contains(TabId::new(10)) && state.find_group(&key).is_none());
}
