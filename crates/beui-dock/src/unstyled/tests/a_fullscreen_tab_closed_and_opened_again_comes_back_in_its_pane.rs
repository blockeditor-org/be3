use super::*;
use crate::unstyled::state::DockFullscreen;

#[test]
fn a_fullscreen_tab_closed_and_opened_again_comes_back_in_its_pane() {
    let mut state = DockState::new([TabId::new(1), TabId::new(2)]);
    state.set_fullscreen(Some(DockFullscreen {
        tab: TabId::new(2),
        area: None,
    }));

    state.close(TabId::new(2));
    assert_eq!(state.fullscreen(), None, "a closed tab is no longer shown");

    let leaf = state.leaves(state.main())[0];
    state.insert(leaf, 1, TabId::new(2));
    assert_eq!(
        state.fullscreen(),
        None,
        "a tab opened again under the same id is not fullscreen"
    );
}
