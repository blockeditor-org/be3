use super::*;

#[test]
fn switching_walks_the_tabs_from_the_one_shown_last() {
    let mut state = DockState::new([TabId::new(1), TabId::new(2)]);
    let leaf = state.leaves(state.main())[0];
    state.split(leaf, Side::Right, 0.5, vec![TabId::new(3)]);
    let rect = Rect::from_min_size(Pos2::ZERO, FLOATING_SIZE);
    state.open_window(rect, vec![TabId::new(4)]);
    for tab in [1, 3, 4, 2] {
        state.show(TabId::new(tab));
    }

    state.begin_switch(false);
    let switch = state.switch().expect("a switch has begun");
    assert_eq!(
        switch.tabs,
        [2, 4, 3, 1].map(TabId::new),
        "the switcher lists every pane's and window's tabs, the one shown last first"
    );
    assert_eq!(
        switch.chosen,
        TabId::new(4),
        "beginning chooses the tab shown before the current one"
    );
    assert_eq!(
        state.focused_tab(),
        Some(TabId::new(2)),
        "nothing is shown until the switch is committed"
    );

    state.step_switch(false);
    state.commit_switch();
    assert!(state.switch().is_none(), "committing ends the switch");
    assert_eq!(
        state.focused_tab(),
        Some(TabId::new(3)),
        "two steps forward show the tab two back"
    );
    assert_eq!(state.recent_tabs()[..2], [TabId::new(3), TabId::new(2)]);

    state.begin_switch(true);
    assert_eq!(
        state.switch().map(|switch| switch.chosen),
        Some(TabId::new(1)),
        "beginning backwards chooses the tab shown longest ago"
    );
    state.step_switch(true);
    state.step_switch(false);
    state.step_switch(false);
    assert_eq!(
        state.switch().map(|switch| switch.chosen),
        Some(TabId::new(3)),
        "stepping past either end wraps around"
    );
    state.commit_switch();
    assert_eq!(state.focused_tab(), Some(TabId::new(3)));
}
