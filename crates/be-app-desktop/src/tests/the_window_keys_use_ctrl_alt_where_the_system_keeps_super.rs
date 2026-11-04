use be_sdk_beui::SuperKeptWatch;

use super::*;

#[test]
fn the_window_keys_use_ctrl_alt_where_the_system_keeps_super() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    fixture.test.set_watched::<SuperKeptWatch>(&true);
    fixture.settle();
    let intercepted = fixture.test.intercepted_keys();
    for (modifiers, key) in [
        (Modifiers::LOGO, Key::ArrowLeft),
        (Modifiers::LOGO, Key::D),
        (Modifiers::LOGO, Key::F),
        (Modifiers::ALT, Key::F4),
    ] {
        assert!(
            !intercepted.contains(&KeyChord::new(key, modifiers)),
            "{modifiers:?}+{key:?} is left to the system"
        );
    }
    let ctrl_alt = Modifiers {
        alt: true,
        ..Modifiers::CTRL
    };
    assert!(fixture.test.host_key(ctrl_alt, Key::ArrowLeft));
    fixture.settle();
    assert!(
        fixture
            .window_rect(2)
            .is_some_and(|rect| rect.width() > 320.0),
        "Ctrl+Alt+Left snaps the window"
    );
    assert!(fixture.test.host_key(Modifiers::CTRL, Key::F4));
    fixture.settle();
    assert_eq!(
        fixture.closed_windows(),
        [HostWindowId(2)],
        "and Ctrl+F4 closes it"
    );
}
