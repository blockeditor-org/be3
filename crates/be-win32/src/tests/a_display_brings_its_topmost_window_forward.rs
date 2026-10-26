use super::*;
use crate::focus::display_window;

#[test]
fn a_display_brings_its_topmost_window_forward() {
    let mut snapshot = desk(vec![
        entry(1, EntryKind::Display, Rect::new(0, 0, 1920, 1080)),
        entry(2, EntryKind::Display, Rect::new(1920, 0, 3840, 1080)),
        entry(3, EntryKind::Window, Rect::new(2000, 100, 2600, 700)),
        entry(4, EntryKind::Window, Rect::new(100, 100, 900, 700)),
        entry(5, EntryKind::Window, Rect::new(1800, 100, 2900, 700)),
    ]);
    let z_order = [Handle(77), Handle(40), Handle(50), Handle(30)];
    let window = |snapshot: &Snapshot, display: usize| {
        display_window(snapshot, &snapshot.entries[display], &z_order).map(|entry| entry.id)
    };
    assert_eq!(window(&snapshot, 0), Some(EntryId(4)));
    assert_eq!(window(&snapshot, 1), Some(EntryId(5)));
    snapshot.entries[4].minimized = true;
    assert_eq!(window(&snapshot, 1), Some(EntryId(3)));
    snapshot.entries.truncate(2);
    assert_eq!(window(&snapshot, 1), None);
}
