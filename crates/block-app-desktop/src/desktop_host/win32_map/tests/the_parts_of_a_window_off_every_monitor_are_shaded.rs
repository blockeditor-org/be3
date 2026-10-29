use super::*;

#[test]
fn the_parts_of_a_window_off_every_monitor_are_shaded() {
    let monitors = [Rect::new(0, 0, 2000, 1000), Rect::new(2000, 0, 3000, 800)];
    assert_eq!(
        off_screen(Rect::new(1800, 600, 2400, 1200), &monitors),
        [
            Rect::new(1800, 1000, 2400, 1200),
            Rect::new(2000, 800, 2400, 1000)
        ]
    );
    assert!(off_screen(Rect::new(100, 100, 900, 700), &monitors).is_empty());
    assert_eq!(
        off_screen(Rect::new(-200, 100, 300, 300), &monitors),
        [Rect::new(-200, 100, 0, 300)]
    );

    let entry = window(2, Rect::new(1800, 600, 2400, 1200));
    let snapshot = snapshot(vec![entry.clone()]);
    let parts = off_screen_parts(&snapshot, &entry);
    let rounded: Vec<[i32; 4]> = parts
        .iter()
        .map(|part| {
            [part.min.x, part.min.y, part.max.x, part.max.y].map(|at| (at * 600.0).round() as i32)
        })
        .collect();
    assert_eq!(rounded, [[0, 400, 600, 600], [200, 200, 600, 400]]);

    let display = Entry {
        kind: EntryKind::Display,
        ..entry.clone()
    };
    assert!(off_screen_parts(&snapshot, &display).is_empty());
    let minimized = Entry {
        minimized: true,
        ..entry
    };
    assert!(off_screen_parts(&snapshot, &minimized).is_empty());
}
