use super::*;

#[test]
fn the_secure_desktop_suspends_focus_and_refuses_it() {
    let frame = Rect::new(100, 100, 900, 700);
    let mut snapshot = desk(vec![entry(3, EntryKind::Window, frame)]);
    let mut mode = FocusMode::new();
    mode.start(&snapshot, EntryId(3), None).unwrap();
    mode.step(&snapshot, Foreground::Entry(EntryId(3)), &[]);

    snapshot.input_desktop = InputDesktop::Other("Winlogon".to_owned());
    let out = mode.step(&snapshot, Foreground::None, &[Event::DesktopSwitched]);
    assert_eq!(out.events, [FocusEvent::Suspended(EntryId(3))]);
    assert!(out.commands.is_empty());
    assert_eq!(
        mode.step(&snapshot, Foreground::None, &[]),
        Output::default()
    );
    assert_eq!(
        FocusMode::new().start(&snapshot, EntryId(3), None),
        Err(FocusError::SecureDesktop)
    );

    snapshot.input_desktop = InputDesktop::Default;
    let out = mode.step(
        &snapshot,
        Foreground::Entry(EntryId(3)),
        &[Event::DesktopSwitched],
    );
    assert_eq!(out.events, [FocusEvent::Resumed(EntryId(3))]);
    assert_eq!(out.commands, [Command::Clip(frame)]);

    let off = desk(vec![entry(
        6,
        EntryKind::Window,
        Rect::new(-900, -900, -100, -100),
    )]);
    assert_eq!(
        FocusMode::new().start(&off, EntryId(6), None),
        Err(FocusError::OffScreen)
    );
}
