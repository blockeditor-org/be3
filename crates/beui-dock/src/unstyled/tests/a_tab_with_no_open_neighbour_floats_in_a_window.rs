use super::*;

#[test]
fn a_tab_with_no_open_neighbour_floats_in_a_window() {
    let mut state = DockState::default();
    let layout = |left: &[u64], right: &[u64]| {
        spec(split(
            "root",
            Direction::Horizontal,
            0.5,
            pane("left", left),
            pane("right", right),
        ))
    };
    state.reconcile(&layout(&[1], &[]));
    state.reconcile(&layout(&[], &[2]));

    let windows = state.windows();
    assert_eq!(
        windows.len(),
        1,
        "a tab with nowhere to go opens in a window"
    );
    assert_eq!(
        state.surface_tabs(windows[0]),
        vec![TabId::new(2)],
        "the window holds the tab"
    );
}
