use super::*;

#[test]
fn the_secure_desktop_covers_displays_and_refusals_stay_on_their_entry() {
    let display = Entry {
        id: EntryId(1),
        kind: EntryKind::Display,
        frame: Rect::new(0, 0, 2000, 1000),
        ..Entry::default()
    };
    let entry = window(2, Rect::new(0, 0, 100, 100));
    let mut snapshot = snapshot(vec![display.clone(), entry.clone()]);
    assert_eq!(surface_notice(&snapshot, &display, None), None);
    let refused = SurfaceNotice::Elevated;
    assert_eq!(
        surface_notice(&snapshot, &entry, Some(&refused)),
        Some(SurfaceNotice::Elevated)
    );
    snapshot.input_desktop = InputDesktop::Other("Winlogon".to_owned());
    assert_eq!(
        surface_notice(&snapshot, &display, None),
        Some(SurfaceNotice::SecureDesktop)
    );
    assert_eq!(surface_notice(&snapshot, &entry, None), None);
    assert_eq!(SurfaceNotice::SecureDesktop.text(), SECURE_DESKTOP);
    assert_eq!(SurfaceNotice::Elevated.text(), ELEVATED);
    assert_eq!(
        SurfaceNotice::Focus(FocusError::ForegroundRefused).text(),
        "Windows refused to bring the window to the front."
    );
}
