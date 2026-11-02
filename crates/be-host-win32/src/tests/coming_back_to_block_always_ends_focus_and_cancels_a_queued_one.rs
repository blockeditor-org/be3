use super::*;

#[test]
fn coming_back_to_block_always_ends_focus_and_cancels_a_queued_one() {
    let frame = Rect::new(100, 100, 900, 700);
    let snapshot = desk(vec![entry(3, EntryKind::Window, frame)]);
    let mut mode = FocusMode::new();
    mode.set_hotkey(true);

    let since = mode.returns();
    mode.start(&snapshot, EntryId(3), None, since).unwrap();
    assert_eq!(
        mode.step(&snapshot, Foreground::Own, &[]),
        Output::default()
    );
    assert_eq!(
        mode.step(&snapshot, Foreground::Other, &[]),
        Output::default()
    );
    let out = mode.step(&snapshot, Foreground::Own, &[]);
    assert_eq!(
        out.events,
        [FocusEvent::Ended {
            entry: EntryId(3),
            reason: EndReason::Returned
        }]
    );
    assert_eq!(mode.focused(), None);
    assert_eq!(
        mode.start(&snapshot, EntryId(3), None, since),
        Err(FocusError::Returned)
    );

    let since = mode.returns();
    mode.start(&snapshot, EntryId(3), None, since).unwrap();
    mode.step(&snapshot, Foreground::Entry(EntryId(3)), &[]);
    let out = mode.observe(Foreground::Own);
    assert_eq!(out.commands, [Command::Unclip]);
    assert_eq!(
        mode.start(&snapshot, EntryId(3), None, since),
        Err(FocusError::Returned)
    );

    let since = mode.returns();
    mode.start(&snapshot, EntryId(3), None, since).unwrap();
    for _ in 1..ARRIVAL_TICKS {
        let out = mode.step(&snapshot, Foreground::Own, &[Event::Tick]);
        assert_eq!(out, Output::default());
    }
    let out = mode.step(&snapshot, Foreground::Own, &[Event::Tick]);
    assert_eq!(
        out.events,
        [FocusEvent::Ended {
            entry: EntryId(3),
            reason: EndReason::NeverArrived
        }]
    );
}
