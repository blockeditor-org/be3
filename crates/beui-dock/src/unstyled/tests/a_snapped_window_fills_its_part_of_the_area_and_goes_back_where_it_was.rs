use super::*;

#[test]
fn a_snapped_window_fills_its_part_of_the_area_and_goes_back_where_it_was() {
    let _root = RootScope::new();
    let tab = TabId::new;
    let mut state = DockState::new([tab(1)]);
    let floating = Rect::from_min_size(Pos2::new(100.0, 80.0), FLOATING_SIZE);
    state.open_window(floating, vec![tab(2)]);
    let layout = DockingLayout::<TabId>::new();
    layout.restore(DockingSnapshot::new(
        state,
        [(tab(1), tab(1)), (tab(2), tab(2))],
    ));
    let area = Rect::from_min_size(Pos2::new(0.0, 40.0), Vec2::new(1000.0, 600.0));
    assert_eq!(layout.window_snap(&tab(2)), None);

    layout.snap_window(&tab(2), DockSnap::Left, area);
    assert_eq!(
        layout.window_rect(&tab(2)),
        Some(Rect::from_min_max(Pos2::new(8.0, 48.0), Pos2::new(500.0, 632.0))),
        "snapped left, the window fills the left half of the area, keeping its grips inside it"
    );
    assert_eq!(layout.window_snap(&tab(2)), Some(DockSnap::Left));

    layout.snap_window(&tab(2), DockSnap::Right, area);
    assert_eq!(
        layout.window_rect(&tab(2)),
        Some(Rect::from_min_max(Pos2::new(500.0, 48.0), Pos2::new(992.0, 632.0))),
        "snapped right, it fills the right half"
    );
    layout.snap_window(&tab(2), DockSnap::Fill, area);
    assert_eq!(
        layout.window_rect(&tab(2)),
        Some(area.shrink(HANDLE_REACH)),
        "filled, it fills the area"
    );
    assert_eq!(layout.window_snap(&tab(2)), Some(DockSnap::Fill));

    assert!(layout.unsnap_window(&tab(2)));
    assert_eq!(
        layout.window_rect(&tab(2)),
        Some(floating),
        "restored, it goes back where it was before it was first snapped"
    );
    assert_eq!(layout.window_snap(&tab(2)), None);
    assert!(!layout.unsnap_window(&tab(2)), "a window not snapped has nothing to go back to");

    layout.snap_window(&tab(2), DockSnap::Right, area);
    layout.move_window(&tab(2), Pos2::new(120.0, 90.0));
    assert_eq!(
        layout.window_snap(&tab(2)),
        None,
        "a snapped window moved away is no longer snapped"
    );
    assert!(!layout.unsnap_window(&tab(2)));
    assert_eq!(
        layout.window_rect(&tab(2)).map(|rect| rect.min),
        Some(Pos2::new(120.0, 90.0)),
        "and stays where it was moved"
    );
}
