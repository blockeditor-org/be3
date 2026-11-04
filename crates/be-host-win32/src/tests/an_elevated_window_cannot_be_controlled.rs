use super::*;

#[test]
fn an_elevated_window_cannot_be_controlled() {
    assert_eq!(
        control(Some(IntegrityLevel::MEDIUM), Some(IntegrityLevel::LOW)),
        Control::Full
    );
    assert_eq!(
        control(Some(IntegrityLevel::MEDIUM), Some(IntegrityLevel::MEDIUM)),
        Control::Full
    );
    assert_eq!(
        control(Some(IntegrityLevel::MEDIUM), Some(IntegrityLevel::HIGH)),
        Control::Elevated
    );
    assert_eq!(
        control(Some(IntegrityLevel::HIGH), Some(IntegrityLevel::HIGH)),
        Control::Full
    );
    assert_eq!(control(Some(IntegrityLevel::HIGH), None), Control::Elevated);
    assert_eq!(
        control(None, Some(IntegrityLevel::MEDIUM_PLUS)),
        Control::Elevated
    );

    let desktop = FakeDesktop::new();
    desktop.add(window(1, "Notepad"));
    desktop.add(window(2, "Administrator: Terminal"));
    desktop.add(window(3, "Task Manager"));
    desktop
        .processes
        .borrow_mut()
        .get_mut(&102)
        .unwrap()
        .integrity = Some(IntegrityLevel::HIGH);
    desktop
        .processes
        .borrow_mut()
        .get_mut(&103)
        .unwrap()
        .integrity = None;
    let mut model = Model::new();
    model.update(&desktop, &[]);
    let controls: Vec<Control> = model
        .snapshot()
        .entries
        .iter()
        .map(|entry| entry.control)
        .collect();
    assert_eq!(
        controls,
        [Control::Full, Control::Elevated, Control::Elevated]
    );
}
