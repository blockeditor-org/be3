use super::*;

#[test]
fn the_window_actions_act_on_the_focused_window_until_the_shell_disables_them() {
    let mut document = beui_core::document::Document::new();
    beui_core::current::with_reactive_scope(&mut document, acts_on_the_focused_window);
}

fn acts_on_the_focused_window() {
    let _root = RootScope::new();
    let tab = TabId::new;
    let mut state = DockState::new([tab(1)]);
    let rect = Rect::from_min_size(Pos2::new(100.0, 80.0), FLOATING_SIZE);
    for window in [2, 3, 4] {
        state.open_window(rect, vec![tab(window)]);
    }
    for shown in [2, 3, 4] {
        state.show(tab(shown));
    }
    let layout = DockHostLayout::new();
    layout.restore(state);
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 760.0));
    layout.set_areas(vec![DockArea { screen, work: area }]);
    let locked = create_signal(false);
    let actions = layout.window_actions(create_memo(move || locked.get()));
    let maximize = actions.maximize.detached();
    let restore = actions.restore.detached();
    let hide = actions.hide.detached();
    let next_area = actions.next_area.detached();
    let switch_next = actions.switch_next.detached();
    let fullscreen = actions.fullscreen.detached();

    assert!(!next_area.is_enabled(), "one area has nowhere to move to");
    assert!(!restore.is_enabled(), "nor has a window not snapped");
    assert!(maximize.run());
    assert_eq!(
        layout.window_rect(tab(4)),
        Some(area.shrink(HANDLE_REACH)),
        "maximize fills the focused window's area"
    );
    assert!(
        !hide.is_enabled(),
        "a snapped window is restored before it hides"
    );
    assert!(restore.run());
    assert_eq!(layout.window_rect(tab(4)), Some(rect));
    assert!(hide.run());
    assert!(layout.is_hidden(tab(4)));
    assert_eq!(layout.focused(), Some(tab(3)));

    assert!(fullscreen.run());
    assert_eq!(
        layout.fullscreen(),
        Some(tab(3)),
        "full screen shows the focused window's tab full screen"
    );
    assert_eq!(
        layout
            .state()
            .with_untracked(|state| state.fullscreen().and_then(|shown| shown.area)),
        Some(screen),
        "over its whole display, the bars kept clear of windows included"
    );
    assert!(fullscreen.run());
    assert_eq!(layout.fullscreen(), None, "and again takes it out");

    assert!(switch_next.run());
    assert_eq!(
        layout.focused(),
        Some(tab(2)),
        "the switcher goes to the window shown before, past the hidden one"
    );
    assert!(switch_next.run());
    assert_eq!(layout.focused(), Some(tab(3)));

    locked.set(true);
    for action in [&maximize, &hide, &switch_next, &fullscreen] {
        assert!(
            !action.is_enabled(),
            "{} is off while disabled",
            action.id()
        );
    }
}
