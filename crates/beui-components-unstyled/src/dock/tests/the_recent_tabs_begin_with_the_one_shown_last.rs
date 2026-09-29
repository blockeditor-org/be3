use super::*;

#[test]
fn the_recent_tabs_begin_with_the_one_shown_last() {
    let mut state = DockState::new((1..=3).map(TabId::new));
    let leaf = state.leaves(state.main())[0];
    state.split(leaf, Side::Right, 0.5, vec![TabId::new(4)]);

    state.show(TabId::new(2));
    state.show(TabId::new(4));
    state.show(TabId::new(3));

    assert_eq!(
        state.recent_tabs(),
        vec![TabId::new(3), TabId::new(4), TabId::new(2), TabId::new(1)],
        "the tabs are listed from the one shown last to the one never shown"
    );

    state.remove(TabId::new(3));
    assert_eq!(
        state.recent_tabs()[..2],
        [TabId::new(4), TabId::new(2)],
        "a closed tab leaves the list and the one shown before it leads"
    );
}
