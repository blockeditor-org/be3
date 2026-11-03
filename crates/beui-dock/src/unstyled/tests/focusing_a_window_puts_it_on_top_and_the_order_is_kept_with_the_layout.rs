use super::*;

fn stacked(state: &DockState) -> Vec<Vec<TabId>> {
    state
        .windows()
        .into_iter()
        .map(|window| state.surface_tabs(window))
        .collect()
}

#[test]
fn focusing_a_window_puts_it_on_top_and_the_order_is_kept_with_the_layout() {
    let tab = TabId::new;
    let mut state = DockState::new([tab(1)]);
    for (at, opened) in [2, 3, 4].into_iter().enumerate() {
        state.open_window(
            Rect::from_min_size(Pos2::new(40.0 * at as f32, 40.0), FLOATING_SIZE),
            vec![tab(opened)],
        );
    }
    assert_eq!(
        stacked(&state),
        vec![vec![tab(2)], vec![tab(3)], vec![tab(4)]],
        "a window opens on top"
    );

    state.show(tab(2));
    assert_eq!(
        stacked(&state),
        vec![vec![tab(3)], vec![tab(4)], vec![tab(2)]],
        "showing a tab puts its window on top"
    );
    state.show(tab(1));
    assert_eq!(
        stacked(&state),
        vec![vec![tab(3)], vec![tab(4)], vec![tab(2)]],
        "focusing the main surface leaves the windows as they are"
    );

    let mut spec = state.spec();
    let restored = DockState::from_spec(&spec);
    assert_eq!(
        stacked(&restored),
        stacked(&state),
        "the layout keeps the order of its windows"
    );

    spec.focus = Some(tab(3));
    let restored = DockState::from_spec(&spec);
    assert_eq!(restored.focused_tab(), Some(tab(3)));
    assert_eq!(
        stacked(&restored),
        vec![vec![tab(4)], vec![tab(2)], vec![tab(3)]],
        "a layout read back puts the focused window on top"
    );
}
