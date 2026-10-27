use super::*;

#[test]
fn a_display_keeps_focus_and_clips_to_its_monitor() {
    let monitor = Rect::new(1920, 0, 3840, 1080);
    let snapshot = desk(vec![
        entry(2, EntryKind::Display, monitor),
        entry(3, EntryKind::Window, Rect::new(2000, 100, 2600, 700)),
        entry(4, EntryKind::Window, Rect::new(100, 100, 900, 700)),
    ]);
    let mut mode = FocusMode::new();
    mode.set_hotkey(true);
    mode.start(&snapshot, EntryId(2), Some(Point::new(10, 20)), 0)
        .unwrap();
    assert_eq!(
        mode.step(&snapshot, Foreground::Own, &[]),
        Output::default()
    );
    let out = mode.step(&snapshot, Foreground::Other, &[]);
    assert_eq!(
        out.commands,
        [
            Command::Clip(monitor),
            Command::MoveCursor(Point::new(1930, 20))
        ]
    );
    let out = mode.step(
        &snapshot,
        Foreground::Entry(EntryId(4)),
        &[Event::Foreground(Handle(40))],
    );
    assert_eq!(out.commands, [Command::Clip(monitor)]);
    assert!(out.events.is_empty());
    assert_eq!(mode.focused().unwrap().entry, EntryId(2));

    let out = mode.step(&desk(vec![]), Foreground::Other, &[Event::DisplaysChanged]);
    assert_eq!(
        out.events,
        [FocusEvent::Ended {
            entry: EntryId(2),
            reason: EndReason::Closed
        }]
    );
}
