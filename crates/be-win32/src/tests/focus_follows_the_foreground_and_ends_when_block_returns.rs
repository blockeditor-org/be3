use super::*;

#[test]
fn focus_follows_the_foreground_and_ends_when_block_returns() {
    let snapshot = desk(vec![
        entry(3, EntryKind::Window, Rect::new(100, 100, 900, 700)),
        entry(4, EntryKind::Window, Rect::new(1000, 100, 1500, 600)),
    ]);
    let mut mode = FocusMode::new();
    mode.set_hotkey(true);
    mode.start(&snapshot, EntryId(3), None, 0).unwrap();
    mode.step(&snapshot, Foreground::Own, &[]);
    let out = mode.step(&snapshot, Foreground::Entry(EntryId(3)), &[]);
    assert!(out.events.is_empty());

    let out = mode.step(
        &snapshot,
        Foreground::Entry(EntryId(4)),
        &[Event::Foreground(Handle(40))],
    );
    assert_eq!(
        out.events,
        [FocusEvent::Followed {
            from: EntryId(3),
            to: EntryId(4)
        }]
    );
    assert_eq!(
        out.commands,
        [Command::Clip(Rect::new(1000, 100, 1500, 600))]
    );

    let out = mode.step(&snapshot, Foreground::Other, &[]);
    assert_eq!(out, Output::default());
    assert_eq!(mode.focused().unwrap().entry, EntryId(4));

    let out = mode.step(&snapshot, Foreground::Own, &[]);
    assert_eq!(out.commands, [Command::Unclip]);
    assert_eq!(
        out.events,
        [FocusEvent::Ended {
            entry: EntryId(4),
            reason: EndReason::Returned
        }]
    );

    let since = mode.returns();
    mode.start(&snapshot, EntryId(3), None, since).unwrap();
    let out = mode.start(&snapshot, EntryId(4), None, since).unwrap();
    assert_eq!(
        out.events,
        [
            FocusEvent::Ended {
                entry: EntryId(3),
                reason: EndReason::Replaced
            },
            FocusEvent::Started(EntryId(4))
        ]
    );
}
