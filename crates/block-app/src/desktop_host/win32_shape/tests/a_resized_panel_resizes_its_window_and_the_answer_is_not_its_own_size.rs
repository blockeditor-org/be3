use super::*;

#[test]
fn a_resized_panel_resizes_its_window_and_the_answer_is_not_its_own_size() {
    let mut snapshot = snapshot(vec![window(1, Rect::new(100, 100, 900, 700))]);
    let mut shaping = Shaping::default();
    shaping.observe(&snapshot);
    let entry = snapshot.entries[0].clone();
    assert_eq!(shaping.own_size(&entry), (800, 600));
    assert_eq!(
        shaping.placed(&snapshot, EntryId(1), Vec2::new(400.0, 300.0)),
        None
    );
    assert_eq!(
        shaping.placed(&snapshot, EntryId(1), Vec2::new(400.4, 300.6)),
        None
    );
    assert_eq!(
        shaping.placed(&snapshot, EntryId(1), Vec2::new(500.0, 300.0)),
        Some((1000, 600))
    );
    assert_eq!(
        shaping.placed(&snapshot, EntryId(1), Vec2::new(500.0, 300.0)),
        None
    );
    assert_eq!(
        shaping.placed(&snapshot, EntryId(1), Vec2::new(550.0, 320.0)),
        Some((1100, 640))
    );
    snapshot.entries[0].frame = Rect::new(100, 100, 1100, 700);
    shaping.observe(&snapshot);
    assert_eq!(shaping.own_size(&snapshot.entries[0]), (800, 600));
    snapshot.entries[0].frame = Rect::new(100, 100, 1200, 740);
    shaping.observe(&snapshot);
    shaping.observe(&snapshot);
    assert_eq!(shaping.own_size(&snapshot.entries[0]), (800, 600));
    assert_eq!(
        shaping.placed(&snapshot, EntryId(1), Vec2::new(400.0, 300.0)),
        Some((800, 600))
    );
}
