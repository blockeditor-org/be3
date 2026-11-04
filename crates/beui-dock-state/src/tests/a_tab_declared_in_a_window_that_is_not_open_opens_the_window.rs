use super::*;

#[test]
fn a_tab_declared_in_a_window_that_is_not_open_opens_the_window() {
    let mut state = DockState::default();
    state.reconcile(&spec(pane("tabs", &[1])));
    let mut later = spec(pane("tabs", &[1]));
    later.windows.push(DockSpecWindow {
        key: "inspector".to_owned(),
        rect: Rect::from_min_size(Pos2::new(40.0, 30.0), Vec2::new(320.0, 240.0)),
        root: Some(pane("inspector", &[2])),
    });
    state.reconcile(&later);

    let windows = state.windows();
    assert_eq!(windows.len(), 1, "the declared window opens for its tab");
    assert_eq!(
        state.surface_tabs(windows[0]),
        vec![TabId::new(2)],
        "the window holds the tab"
    );
    assert_eq!(
        state.window_rect(windows[0]).map(|rect| rect.min),
        Some(Pos2::new(40.0, 30.0)),
        "the window opens where the code put it"
    );
}
