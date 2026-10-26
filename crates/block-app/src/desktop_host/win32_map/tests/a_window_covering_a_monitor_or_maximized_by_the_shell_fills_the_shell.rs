use super::*;

#[test]
fn a_window_covering_a_monitor_or_maximized_by_the_shell_fills_the_shell() {
    let area = block_plugin_api::Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 720.0));
    let covering = window(5, Rect::new(2000, 0, 3000, 800));
    let mut maximized = window(6, Rect::new(0, 0, 2000, 960));
    let mut snapshot = snapshot(vec![covering.clone(), maximized.clone()]);
    let mut shaping = Shaping::default();
    shaping.observe(&snapshot);
    assert_eq!(host_window(&snapshot, &covering, &shaping).fullscreen, None);
    assert!(shaping.set_area(area));
    assert!(!shaping.set_area(area));
    let host = host_window(&snapshot, &covering, &shaping);
    assert_eq!(host.size, Vec2::new(1000.0, 800.0));
    assert_eq!(host.fullscreen, Some(area));
    assert_eq!(host_window(&snapshot, &maximized, &shaping).fullscreen, None);
    snapshot.entries[1].maximized = true;
    maximized.maximized = true;
    assert_eq!(host_window(&snapshot, &maximized, &shaping).fullscreen, None);
    assert_eq!(shaping.enter_fullscreen(&snapshot, EntryId(6)), None);
    assert_eq!(host_window(&snapshot, &maximized, &shaping).fullscreen, Some(area));
    assert_eq!(shaping.enter_fullscreen(&snapshot, EntryId(5)), None);
}
