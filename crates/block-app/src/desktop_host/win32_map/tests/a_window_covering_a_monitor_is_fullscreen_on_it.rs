use super::*;

#[test]
fn a_window_covering_a_monitor_is_fullscreen_on_it() {
    let entry = window(5, Rect::new(2000, 0, 3000, 800));
    let snapshot = snapshot(vec![entry.clone()]);
    let host = host_window(&snapshot, &entry);
    assert_eq!(host.size, Vec2::new(1000.0, 800.0));
    assert_eq!(
        host.fullscreen,
        Some(block_plugin_api::Rect::from_min_size(
            Pos2::new(2000.0, 0.0),
            Vec2::new(1000.0, 800.0)
        ))
    );
}
