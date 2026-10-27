use super::*;

#[test]
fn a_window_that_resizes_itself_is_followed() {
    let mut snapshot = snapshot(vec![window(1, Rect::new(100, 100, 900, 700))]);
    let mut shaping = Shaping::default();
    shaping.observe(&snapshot);
    assert_eq!(
        shaping.placed(&snapshot, EntryId(1), Vec2::new(200.0, 100.0)),
        Some((400, 200))
    );
    snapshot.entries[0].frame = Rect::new(100, 100, 700, 500);
    shaping.observe(&snapshot);
    assert_eq!(shaping.own_size(&snapshot.entries[0]), (600, 400));
    assert_eq!(
        shaping.placed(&snapshot, EntryId(1), Vec2::new(300.0, 200.0)),
        None
    );
    snapshot.entries.clear();
    shaping.observe(&snapshot);
    let gone = window(1, Rect::new(0, 0, 10, 10));
    assert_eq!(shaping.own_size(&gone), (10, 10));
}
