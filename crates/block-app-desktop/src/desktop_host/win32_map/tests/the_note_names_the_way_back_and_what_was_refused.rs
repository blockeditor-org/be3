use super::*;

#[test]
fn the_note_names_the_way_back_and_what_was_refused() {
    let display = Entry {
        id: EntryId(1),
        kind: EntryKind::Display,
        frame: Rect::new(0, 0, 2000, 1000),
        ..Entry::default()
    };
    let entry = window(2, Rect::new(0, 0, 100, 100));
    let registered = HotkeyState::Registered(Hotkey::DEFAULT);
    let mut notes = Notes {
        hotkey: registered.clone(),
        ..Notes::default()
    };
    assert_eq!(notes.note(&entry), None);
    notes.hovered = Some(entry.id);
    assert_eq!(
        notes.note(&entry).as_deref(),
        Some("Click to use this window. Ctrl+Alt+Space comes back to Block")
    );
    notes
        .refused
        .insert(entry.id, Refusal::Focus(FocusError::ForegroundRefused));
    assert_eq!(
        notes.note(&entry).as_deref(),
        Some("Windows refused to bring the window to the front.")
    );
    notes.focused = Some(entry.id);
    assert_eq!(
        notes.note(&entry).as_deref(),
        Some("Ctrl+Alt+Space comes back to Block")
    );
    assert_eq!(notes.note(&display), None);
    notes.hovered = Some(display.id);
    notes.hotkey = HotkeyState::Refused(HotkeyError::Taken(Hotkey::DEFAULT));
    assert_eq!(
        notes.note(&display).as_deref(),
        Some("Click to use the window under the pointer. Alt+Tab comes back to Block")
    );
    assert_eq!(
        focused_note(&notes.hotkey),
        "Alt+Tab comes back to Block. The mouse is free, since Ctrl+Alt+Space is taken by another program"
    );
    assert_eq!(
        focused_note(&HotkeyState::Waiting),
        "Alt+Tab comes back to Block. The mouse is free, since the shortcut back isn't ready yet"
    );
    assert_eq!(Refusal::Elevated.text(), ELEVATED);

    notes.hotkey = registered;
    let snapshot = snapshot(vec![display.clone(), entry.clone()]);
    let listed = listed(&snapshot, &[], &Shaping::default(), &notes, &ProgramNames::default());
    assert_eq!(
        listed[1].note.as_deref(),
        Some("Ctrl+Alt+Space comes back to Block")
    );
}
