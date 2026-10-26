use super::*;

#[test]
fn focus_ends_or_follows_when_its_window_closes_or_minimizes() {
    let mut snapshot = desk(vec![
        entry(3, EntryKind::Window, Rect::new(100, 100, 900, 700)),
        entry(4, EntryKind::Window, Rect::new(1000, 100, 1500, 600)),
    ]);
    snapshot.entries[0].minimized = true;
    let mut mode = FocusMode::new();
    mode.start(&snapshot, EntryId(3), None).unwrap();
    let out = mode.step(&snapshot, Foreground::Entry(EntryId(3)), &[]);
    assert_eq!(out, Output::default());
    snapshot.entries[0].minimized = false;
    let out = mode.step(
        &snapshot,
        Foreground::Entry(EntryId(3)),
        &[Event::Restored(Handle(30))],
    );
    assert_eq!(
        out.commands,
        [
            Command::Clip(Rect::new(100, 100, 900, 700)),
            Command::MoveCursor(Point::new(500, 400))
        ]
    );

    snapshot.entries[0].minimized = true;
    let out = mode.step(&snapshot, Foreground::Other, &[]);
    assert_eq!(
        out.events,
        [FocusEvent::Ended {
            entry: EntryId(3),
            reason: EndReason::Minimized
        }]
    );

    snapshot.entries[0].minimized = false;
    mode.start(&snapshot, EntryId(3), None).unwrap();
    mode.step(&snapshot, Foreground::Entry(EntryId(3)), &[]);
    let closed = desk(vec![snapshot.entries[1].clone()]);
    let out = mode.step(&closed, Foreground::Entry(EntryId(4)), &[]);
    assert_eq!(
        out.events,
        [FocusEvent::Followed {
            from: EntryId(3),
            to: EntryId(4)
        }]
    );

    let out = mode.step(&desk(vec![]), Foreground::Other, &[]);
    assert_eq!(out.commands, [Command::Unclip]);
    assert_eq!(
        out.events,
        [FocusEvent::Ended {
            entry: EntryId(4),
            reason: EndReason::Closed
        }]
    );
    assert_eq!(
        mode.start(&desk(vec![]), EntryId(4), None),
        Err(FocusError::NoSuchEntry(EntryId(4)))
    );
}
