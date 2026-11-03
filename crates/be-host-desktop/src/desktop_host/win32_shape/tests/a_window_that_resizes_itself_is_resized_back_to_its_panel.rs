use super::*;

#[test]
fn a_window_that_resizes_itself_is_resized_back_to_its_panel() {
    let mut snapshot = snapshot(vec![window(1, Rect::new(100, 100, 900, 700))]);
    let mut shaping = Shaping::default();
    shaping.observe(&snapshot);
    let panel = Vec2::new(200.0, 100.0);
    assert_eq!(
        shaping.placed(&snapshot, EntryId(1), panel),
        Some((400, 200))
    );
    snapshot.entries[0].frame = Rect::new(100, 100, 500, 300);
    shaping.observe(&snapshot);
    assert_eq!(
        shaping.placed(&snapshot, EntryId(1), panel),
        None,
        "a window that took its panel's size is left at it"
    );
    snapshot.entries[0].frame = Rect::new(100, 100, 700, 500);
    shaping.observe(&snapshot);
    assert_eq!(
        shaping.own_size(&snapshot.entries[0]),
        (600, 400),
        "the size it picked is its own"
    );
    assert_eq!(
        shaping.placed(&snapshot, EntryId(1), panel),
        Some((400, 200)),
        "a window that resizes itself is told its panel's size again"
    );
}
