use super::*;

#[test]
fn a_lost_clip_is_put_back_except_while_the_window_is_dragged() {
    let frame = Rect::new(100, 100, 900, 700);
    let snapshot = desk(vec![entry(3, EntryKind::Window, frame)]);
    let everywhere = Rect::new(0, 0, 3840, 1080);
    let mut mode = FocusMode::new();
    mode.set_hotkey(true);
    mode.start(&snapshot, EntryId(3), None, 0).unwrap();
    assert_eq!(mode.reclip(everywhere), None);
    mode.step(&snapshot, Foreground::Entry(EntryId(3)), &[]);
    assert_eq!(mode.reclip(frame), None);
    assert_eq!(mode.reclip(everywhere), Some(Command::Clip(frame)));

    let out = mode.step(
        &snapshot,
        Foreground::Entry(EntryId(3)),
        &[Event::CaptureChanged],
    );
    assert_eq!(out.commands, [Command::Clip(frame)]);

    mode.step(
        &snapshot,
        Foreground::Entry(EntryId(3)),
        &[Event::MoveSizeStarted(Handle(30))],
    );
    assert_eq!(mode.reclip(everywhere), None);
    let moved = desk(vec![entry(3, EntryKind::Window, Rect::new(200, 100, 1000, 700))]);
    let out = mode.step(
        &moved,
        Foreground::Entry(EntryId(3)),
        &[Event::Moved(Handle(30))],
    );
    assert_eq!(out, Output::default());
    let out = mode.step(
        &moved,
        Foreground::Entry(EntryId(3)),
        &[Event::MoveSizeEnded(Handle(30))],
    );
    assert_eq!(out.commands, [Command::Clip(Rect::new(200, 100, 1000, 700))]);
}
