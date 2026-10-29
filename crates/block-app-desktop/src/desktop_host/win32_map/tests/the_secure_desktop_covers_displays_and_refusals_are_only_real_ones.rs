use super::*;

#[test]
fn the_secure_desktop_covers_displays_and_refusals_are_only_real_ones() {
    let display = Entry {
        id: EntryId(1),
        kind: EntryKind::Display,
        frame: Rect::new(0, 0, 2000, 1000),
        ..Entry::default()
    };
    let entry = window(2, Rect::new(0, 0, 100, 100));
    let mut snapshot = snapshot(vec![display.clone(), entry.clone()]);
    assert!(!secure_notice(&snapshot, &display));
    snapshot.input_desktop = InputDesktop::Other("Winlogon".to_owned());
    assert!(secure_notice(&snapshot, &display));
    assert!(!secure_notice(&snapshot, &entry));

    assert_eq!(
        ended_refusal(be_win32::EndReason::NeverArrived),
        Some(Refusal::Focus(FocusError::ForegroundRefused))
    );
    assert_eq!(ended_refusal(be_win32::EndReason::Hotkey), None);
    assert_eq!(focus_refusal(&FocusError::Returned), None);
    assert_eq!(focus_refusal(&FocusError::OnBlock), None);
    assert_eq!(
        focus_refusal(&FocusError::OffScreen),
        Some(Refusal::Focus(FocusError::OffScreen))
    );
}
