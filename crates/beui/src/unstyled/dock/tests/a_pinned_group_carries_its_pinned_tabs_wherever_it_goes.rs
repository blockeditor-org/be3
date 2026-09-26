use super::*;

#[test]
fn a_pinned_group_carries_its_pinned_tabs_wherever_it_goes() {
    let mut state = DockState::new([TabId::new(1)]);
    let leaf = state.leaves(state.main())[0];
    let layout = DockTree::Tabs {
        entries: vec![
            DockTreeEntry::Tab(TabId::new(10)),
            DockTreeEntry::Tab(TabId::new(11)),
        ],
        active: 0,
        vertical: false,
        sidebar: SIDEBAR_WIDTH,
    };
    let group = state.insert_pinned_group(leaf, 1, &layout);

    state.drop_entry(
        Entry::Group(group),
        DockDrop::Window {
            pos: Pos2::new(40.0, 40.0),
        },
    );

    let window = state.windows()[0];
    assert_eq!(state.surface_tabs(window), vec![TabId::new(10), TabId::new(11)]);
    assert!(state.is_tab_pinned(TabId::new(10)));
}
