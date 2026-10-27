use super::*;

#[test]
fn an_elevated_window_or_the_secure_desktop_takes_no_injected_input() {
    let mut snapshot = Snapshot::default();
    assert_eq!(
        target(&snapshot, &entry(EntryKind::Window, Control::Full)),
        Ok(Target::Window {
            hwnd: 42,
            frame: PixelRect::new(10, 20, 100, 200)
        })
    );
    assert_eq!(
        target(&snapshot, &entry(EntryKind::Window, Control::Elevated)),
        Err(InjectError::Elevated)
    );
    assert_eq!(
        target(&snapshot, &entry(EntryKind::Display, Control::Full)),
        Ok(Target::Display {
            rect: PixelRect::new(10, 20, 100, 200)
        })
    );
    snapshot.input_desktop = InputDesktop::Other("Winlogon".to_owned());
    assert_eq!(
        target(&snapshot, &entry(EntryKind::Display, Control::Full)),
        Err(InjectError::SecureDesktop)
    );
    assert!(InjectError::Elevated.needs_administrator());
    assert!(!InjectError::SecureDesktop.needs_administrator());
}
