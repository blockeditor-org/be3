use super::*;
use crate::unstyled::state::joined_key;

#[test]
fn a_tab_hosting_one_bare_pane_stays_locked_in_its_area() {
    let mut state = DockState::default();
    let declared = sidebar_and_editors(&[1], &[2]);
    state.reconcile(&declared);
    assert!(
        state.is_tab_pinned(TabId::new(2)),
        "an editor starts locked in its area"
    );

    let mut bare = pane_spec("joined:2/canvas");
    bare.entries = vec![DockSpecEntry::Tab(TabId::new(10))];
    bare.bare = true;
    let bare = DockSpecNode::Pane(bare);
    let group = state
        .host_group(TabId::new(2), &joined_key(TabId::new(2)), &bare, None)
        .expect("the hosted dock declares a tab");
    let mut editors = pane_spec("editors");
    editors.locked = true;
    editors.entries = vec![DockSpecEntry::Group {
        key: joined_key(TabId::new(2)),
        pinned: true,
        root: Box::new(bare),
    }];
    let spliced = spec(split(
        "workspace",
        Direction::Horizontal,
        0.25,
        locked("files", &[1]),
        DockSpecNode::Pane(editors),
    ));
    state.reconcile(&spliced);
    assert_eq!(
        state.stands_for(group),
        Some(TabId::new(2)),
        "a group of one bare pane stands for the tab that hosts it"
    );
    let (leaf, _) = state
        .locate(Entry::Group(group))
        .expect("the group is placed");
    assert_eq!(
        state.menu_tab(leaf),
        Some(TabId::new(10)),
        "its pane's bar carries the menu of the bare pane's tab"
    );
    assert!(
        state.is_tab_pinned(TabId::new(2)) && state.at_home(TabId::new(2)),
        "and keeps that tab locked where it was"
    );
    state.set_tab_pinned(TabId::new(2), false);
    assert!(
        !state.is_tab_pinned(TabId::new(2)),
        "unlocking it unlocks the group's place"
    );
}
