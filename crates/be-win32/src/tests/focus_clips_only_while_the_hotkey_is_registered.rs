use super::*;

#[test]
fn focus_clips_only_while_the_hotkey_is_registered() {
    let frame = Rect::new(100, 100, 900, 700);
    let snapshot = desk(vec![entry(3, EntryKind::Window, frame)]);
    let mut mode = FocusMode::new();
    assert!(!mode.clips());
    mode.start(&snapshot, EntryId(3), None, 0).unwrap();
    let out = mode.step(&snapshot, Foreground::Entry(EntryId(3)), &[]);
    assert_eq!(out.commands, [Command::MoveCursor(Point::new(500, 400))]);
    assert_eq!(mode.held_clip(), None);

    assert_eq!(mode.set_hotkey(true), Output::default());
    let out = mode.step(&snapshot, Foreground::Entry(EntryId(3)), &[]);
    assert_eq!(out.commands, [Command::Clip(frame)]);
    assert_eq!(mode.held_clip(), Some(frame));

    assert_eq!(mode.set_hotkey(false).commands, [Command::Unclip]);
    let out = mode.step(&snapshot, Foreground::Entry(EntryId(3)), &[Event::Tick]);
    assert_eq!(out, Output::default());
}
