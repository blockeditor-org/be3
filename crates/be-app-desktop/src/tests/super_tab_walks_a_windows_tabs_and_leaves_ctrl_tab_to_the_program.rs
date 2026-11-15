use super::*;

#[test]
fn super_tab_walks_a_windows_tabs_and_leaves_ctrl_tab_to_the_program() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    fixture.group_two_windows();
    fixture.settle();

    let intercepted = fixture.test.intercepted_keys();
    assert!(
        intercepted.contains(&KeyChord::new(Key::Tab, Modifiers::LOGO)),
        "the desktop asks the host for Super+Tab over the program: {intercepted:?}"
    );
    assert!(
        !intercepted.contains(&KeyChord::new(Key::Tab, Modifiers::CTRL)),
        "and leaves Ctrl+Tab to it"
    );

    let shown = fixture.shown_tab();
    assert!(fixture.test.host_key(Modifiers::LOGO, Key::Tab));
    fixture.settle();
    let next = fixture.shown_tab();
    assert_ne!(next, shown, "Super+Tab shows the window's other tab");
    let back = Modifiers {
        shift: true,
        ..Modifiers::LOGO
    };
    assert!(fixture.test.host_key(back, Key::Tab));
    fixture.settle();
    assert_eq!(fixture.shown_tab(), shown, "Super+Shift+Tab walks back");
}
