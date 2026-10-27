use super::*;

#[test]
fn focus_clips_to_the_window_and_its_popups_and_moves_the_cursor_in() {
    let mut main = entry(3, EntryKind::Window, Rect::new(100, 100, 900, 700));
    main.popups = vec![Popup {
        handle: Handle(99),
        frame: Rect::new(800, 600, 1000, 800),
    }];
    let mut dialog = entry(4, EntryKind::Window, Rect::new(300, 300, 1200, 500));
    dialog.parent = Some(EntryId(3));
    let other = entry(5, EntryKind::Window, Rect::new(-500, 0, 200, 200));
    let mut snapshot = desk(vec![main, dialog, other]);
    let mut mode = FocusMode::new();
    mode.set_hotkey(true);
    let started = mode.start(&snapshot, EntryId(3), None, 0).unwrap();
    assert_eq!(started.events, [FocusEvent::Started(EntryId(3))]);

    let out = mode.step(&snapshot, Foreground::Own, &[]);
    assert_eq!(out, Output::default());
    let out = mode.step(&snapshot, Foreground::Entry(EntryId(3)), &[]);
    let clip = Rect::new(100, 100, 1200, 800);
    assert_eq!(
        out.commands,
        [
            Command::Clip(clip),
            Command::MoveCursor(Point::new(500, 400))
        ]
    );
    assert_eq!(mode.focused().unwrap().clip, Some(clip));

    let out = mode.step(&snapshot, Foreground::Entry(EntryId(3)), &[]);
    assert_eq!(out, Output::default());
    let out = mode.step(
        &snapshot,
        Foreground::Entry(EntryId(4)),
        &[Event::Foreground(Handle(40))],
    );
    assert_eq!(out.commands, [Command::Clip(clip)]);
    assert_eq!(mode.focused().unwrap().entry, EntryId(3));

    snapshot.entries[0].frame = Rect::new(150, 100, 950, 700);
    let out = mode.step(
        &snapshot,
        Foreground::Entry(EntryId(3)),
        &[Event::Moved(Handle(30))],
    );
    assert_eq!(
        out.commands,
        [Command::Clip(Rect::new(150, 100, 1200, 800))]
    );

    let mut mode = FocusMode::new();
    mode.set_hotkey(true);
    mode.start(&snapshot, EntryId(5), Some(Point::new(5000, 10)), 0)
        .unwrap();
    let out = mode.step(&snapshot, Foreground::Entry(EntryId(5)), &[]);
    assert_eq!(
        out.commands,
        [
            Command::Clip(Rect::new(0, 0, 200, 200)),
            Command::MoveCursor(Point::new(199, 10))
        ]
    );
    let out = mode.release(EndReason::Hotkey);
    assert_eq!(out.commands, [Command::Unclip]);
    assert_eq!(
        out.events,
        [FocusEvent::Ended {
            entry: EntryId(5),
            reason: EndReason::Hotkey
        }]
    );
    assert_eq!(mode.focused(), None);
}
