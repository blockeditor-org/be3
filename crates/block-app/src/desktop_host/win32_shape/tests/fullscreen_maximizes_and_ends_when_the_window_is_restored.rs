use super::*;

#[test]
fn fullscreen_maximizes_and_ends_when_the_window_is_restored() {
    let area = block_plugin_api::Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 720.0));
    let mut snapshot = snapshot(vec![window(1, Rect::new(100, 100, 900, 700))]);
    let mut shaping = Shaping::default();
    shaping.set_area(area);
    assert_eq!(
        shaping.enter_fullscreen(&snapshot, EntryId(1)),
        Some(Handle(10))
    );
    shaping.observe(&snapshot);
    assert_eq!(
        shaping.fullscreen_area(&snapshot, &snapshot.entries[0]),
        None
    );
    snapshot.entries[0].maximized = true;
    snapshot.entries[0].frame = Rect::new(0, 0, 2000, 960);
    shaping.observe(&snapshot);
    assert_eq!(
        shaping.fullscreen_area(&snapshot, &snapshot.entries[0]),
        Some(area)
    );
    assert_eq!(
        shaping.leave_fullscreen(&snapshot, EntryId(1)),
        Some(Handle(10))
    );
    assert_eq!(shaping.leave_fullscreen(&snapshot, EntryId(1)), None);
    assert_eq!(
        shaping.fullscreen_area(&snapshot, &snapshot.entries[0]),
        None
    );
    assert_eq!(shaping.enter_fullscreen(&snapshot, EntryId(1)), None);
    snapshot.entries[0].maximized = false;
    shaping.observe(&snapshot);
    snapshot.entries[0].maximized = true;
    shaping.observe(&snapshot);
    assert_eq!(
        shaping.fullscreen_area(&snapshot, &snapshot.entries[0]),
        None
    );
}
