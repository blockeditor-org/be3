use super::*;

#[test]
fn the_secure_desktop_is_any_input_desktop_but_default() {
    assert_eq!(InputDesktop::from_name(Some("default")), InputDesktop::Default);
    assert_eq!(InputDesktop::from_name(Some("Winlogon")), InputDesktop::Other("Winlogon".to_owned()));
    assert_eq!(InputDesktop::from_name(None), InputDesktop::Unavailable);
    assert!(!InputDesktop::Default.secure());
    assert!(InputDesktop::Unavailable.secure());

    let desktop = FakeDesktop::new();
    let mut model = Model::new();
    model.update(&desktop, &[]);
    assert!(!model.snapshot().secure_desktop());

    *desktop.input_desktop.borrow_mut() = InputDesktop::Unavailable;
    assert!(model.update(&desktop, &[Event::DesktopSwitched]));
    assert!(model.snapshot().secure_desktop());

    *desktop.input_desktop.borrow_mut() = InputDesktop::Default;
    assert!(model.update(&desktop, &[Event::DesktopSwitched]));
    assert!(!model.snapshot().secure_desktop());
}
