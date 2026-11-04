use super::*;

#[test]
fn a_window_the_host_focuses_leads_the_window_switcher() {
    let mut fixture = Fixture::with_windows(&[1, 2, 3]);
    let windows = [1, 2, 3].map(|id| host_window(id, id == 1)).to_vec();
    fixture.show_windows(&windows);
    assert_eq!(
        fixture.focused_windows(),
        [HostWindowId(1)],
        "a window its program raises is given the keyboard"
    );

    assert!(fixture.test.host_key(Modifiers::ALT, Key::Tab));
    fixture.settle();
    fixture.test.hold_modifiers(Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture.focused_windows(),
        [HostWindowId(3)],
        "clicking into a window makes it the one Alt+Tab leaves, and the one before it the one it finds"
    );
}
