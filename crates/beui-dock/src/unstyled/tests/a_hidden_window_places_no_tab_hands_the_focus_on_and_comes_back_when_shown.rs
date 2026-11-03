use super::*;

#[test]
fn a_hidden_window_places_no_tab_hands_the_focus_on_and_comes_back_when_shown() {
    let _root = RootScope::new();
    let tab = TabId::new;
    let mut state = DockState::new([tab(1)]);
    let rect = Rect::from_min_size(Pos2::new(40.0, 40.0), FLOATING_SIZE);
    state.open_window(rect, vec![tab(2)]);
    state.open_window(rect, vec![tab(3)]);
    for shown in [2, 3] {
        state.show(tab(shown));
    }
    let layout = DockingLayout::<TabId>::new();
    layout.restore(DockingSnapshot::new(
        state,
        [1, 2, 3].map(|held| (tab(held), tab(held))),
    ));

    layout.hide_window(&tab(3));
    assert!(layout.is_hidden(&tab(3)));
    let shown = layout.state().with(|state| state.is_shown(tab(3), DockMode::Tiled));
    assert!(!shown, "a hidden window's tab is not placed");
    assert_eq!(
        layout.focused(),
        Some(tab(2)),
        "the focus goes to the window shown before it"
    );
    assert_eq!(
        layout.shown_windows(),
        [tab(2)],
        "and only the other window is shown"
    );
    assert_eq!(
        layout.window_rect(&tab(3)),
        Some(rect),
        "the hidden window keeps its place"
    );

    layout.show(&tab(3));
    assert!(!layout.is_hidden(&tab(3)), "showing its tab shows the window");
    assert_eq!(layout.focused(), Some(tab(3)), "with the focus");

    layout.hide_window(&tab(2));
    layout.hide_window(&tab(3));
    assert_eq!(
        layout.focused(),
        Some(tab(1)),
        "with every window hidden, the focus goes back to the dock's own tab"
    );
    layout.hide_window(&tab(1));
    assert!(
        !layout.is_hidden(&tab(1)),
        "a tab outside the floating windows cannot be hidden"
    );
}
