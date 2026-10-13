use super::*;

#[test]
fn cancelling_a_switch_leaves_the_tab_that_was_shown() {
    let mut state = DockState::new((1..=3).map(TabId::new));
    for tab in [2, 3] {
        state.show(TabId::new(tab));
    }
    let before = state.clone();

    state.begin_switch(false);
    state.step_switch(false);
    state.cancel_switch();

    assert!(state.switch().is_none());
    assert_eq!(
        state, before,
        "a cancelled switch shows nothing and leaves the order alone"
    );
}
