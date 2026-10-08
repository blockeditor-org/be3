use super::*;

#[test]
fn a_window_the_host_focuses_leads_the_window_switcher() {
    let mut fixture = Fixture::with_windows(&[1, 2, 3]);
    let windows = [1, 2, 3].map(|id| host_window(id, id == 1)).to_vec();
    fixture.test.linux(LinuxMessage::Windows(windows));
    fixture.settle();
    assert!(
        fixture.focused_windows().is_empty(),
        "a window that already has the keyboard is not given it again"
    );

    assert!(fixture.test.app_key(Modifiers::ALT, Key::Tab));
    fixture.settle();
    fixture.test.hold_modifiers(Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture.focused_windows(),
        [HostWindowId(3)],
        "clicking into a window makes it the one Alt+Tab leaves, and the one before it the one it finds"
    );
}
