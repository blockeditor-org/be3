use super::*;

#[test]
fn a_window_declared_hidden_opens_hidden_and_unfocused_until_shown() {
    let mut state = DockState::default();
    state.reconcile(&spec(pane("tabs", &[1])));
    let focused = state.focused_tab();
    let window = |key: &str, tab: u64, hidden: bool| DockSpecWindow {
        key: key.to_owned(),
        rect: Rect::from_min_size(Pos2::new(40.0, 30.0), Vec2::new(320.0, 240.0)),
        placed: None,
        hidden,
        root: Some(pane(key, &[tab])),
    };
    let mut later = spec(pane("tabs", &[1]));
    later.windows.push(window("picked", 2, true));
    state.reconcile(&later);

    assert!(
        state.is_tab_hidden(TabId::new(2)),
        "a window declared hidden opens hidden"
    );
    assert!(state.shown_windows().is_empty(), "so nothing of it shows");
    assert_eq!(
        state.focused_tab(),
        focused,
        "and it takes no focus from what had it"
    );

    state.reconcile(&later);
    state.show(TabId::new(2));
    assert!(
        !state.is_tab_hidden(TabId::new(2)),
        "showing its tab shows it"
    );
    assert_eq!(state.focused_tab(), Some(TabId::new(2)));

    state.reconcile(&later);
    assert!(
        !state.is_tab_hidden(TabId::new(2)),
        "declaring it hidden again does not hide a window already there"
    );

    later.windows.push(window("shown", 3, false));
    state.reconcile(&later);
    assert_eq!(
        state.focused_tab(),
        Some(TabId::new(3)),
        "a window not declared hidden opens shown and focused"
    );
}
