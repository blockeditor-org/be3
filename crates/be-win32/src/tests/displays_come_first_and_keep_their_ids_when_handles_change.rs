use super::*;

#[test]
fn displays_come_first_and_keep_their_ids_when_handles_change() {
    let desktop = FakeDesktop::new();
    desktop.add(window(1, "main"));
    *desktop.monitors.borrow_mut() = vec![
        monitor(501, "\\\\.\\DISPLAY2", Rect::new(1920, 0, 3840, 1080), false),
        monitor(500, "\\\\.\\DISPLAY1", Rect::new(0, 0, 1920, 1080), true),
    ];
    let mut model = Model::new();
    model.update(&desktop, &[]);
    let first = model.snapshot();
    let kinds: Vec<(EntryKind, &str)> = first
        .entries
        .iter()
        .map(|entry| (entry.kind, entry.title.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            (EntryKind::Display, "Monitor \\\\.\\DISPLAY1"),
            (EntryKind::Display, "Monitor \\\\.\\DISPLAY2"),
            (EntryKind::Window, "main"),
        ]
    );
    assert_eq!(first.entries[1].frame, Rect::new(1920, 0, 3840, 1080));

    *desktop.monitors.borrow_mut() = vec![
        monitor(600, "\\\\.\\DISPLAY1", Rect::new(0, 0, 2560, 1440), true),
        monitor(601, "\\\\.\\DISPLAY2", Rect::new(2560, 0, 4480, 1080), false),
    ];
    model.update(&desktop, &[]);
    assert_eq!(model.snapshot().entries[0].frame, Rect::new(0, 0, 1920, 1080));
    model.update(&desktop, &[Event::DisplaysChanged]);
    let changed = model.snapshot();
    assert_eq!(changed.entries[0].id, first.entries[0].id);
    assert_eq!(changed.entries[0].handle, Handle(600));
    assert_eq!(changed.entries[1].id, first.entries[1].id);
    assert_eq!(changed.monitors[0].rect, Rect::new(0, 0, 2560, 1440));
}
