use super::*;

#[test]
fn an_undeclared_tab_is_closed_and_the_one_shown_before_it_takes_its_place() {
    let mut state = DockState::default();
    state.reconcile(&spec(pane("tabs", &[1, 2, 3])));
    state.show(TabId::new(3));
    state.show(TabId::new(2));
    state.reconcile(&spec(pane("tabs", &[1, 3])));

    assert!(
        !state.contains(TabId::new(2)),
        "a tab the code no longer declares leaves the dock"
    );
    assert_eq!(
        state.focused_tab(),
        Some(TabId::new(3)),
        "the tab shown before it is shown in its place"
    );
}
