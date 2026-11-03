use super::*;

#[test]
fn showing_the_desktop_hides_every_window_and_a_second_press_brings_them_back() {
    let _root = RootScope::new();
    let tab = TabId::new;
    let mut state = DockState::new([tab(1)]);
    let rect = Rect::from_min_size(Pos2::new(40.0, 40.0), FLOATING_SIZE);
    for window in [2, 3, 4] {
        state.open_window(rect, vec![tab(window)]);
    }
    for shown in [3, 4, 2] {
        state.show(tab(shown));
    }
    let layout = DockingLayout::<TabId>::new();
    layout.restore(DockingSnapshot::new(
        state,
        [1, 2, 3, 4].map(|held| (tab(held), tab(held))),
    ));
    layout.hide_window(&tab(4));
    assert!(!layout.shows_desktop());

    layout.show_desktop();
    assert!(layout.shown_windows().is_empty(), "every window is hidden");
    assert!(layout.shows_desktop());

    layout.show_desktop();
    assert!(!layout.shows_desktop());
    let mut shown = layout.shown_windows();
    shown.sort();
    assert_eq!(
        shown,
        [tab(2), tab(3)],
        "a second press brings back what the first hid, and only that"
    );
    assert_eq!(
        layout.focused(),
        Some(tab(2)),
        "with the window shown last in front"
    );

    layout.show_desktop();
    layout.show(&tab(3));
    assert!(
        !layout.shows_desktop(),
        "a window shown meanwhile ends showing the desktop"
    );
    layout.show_desktop();
    assert!(
        layout.shown_windows().is_empty(),
        "so the next press hides again what is shown"
    );
}
