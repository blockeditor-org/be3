use be_sdk_beui::SuperKeptWatch;

use super::*;

#[test]
fn the_window_keys_use_ctrl_alt_where_the_system_keeps_super() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    fixture.test.set_watched::<SuperKeptWatch>(&true);
    fixture.settle();
    let intercepted = fixture.test.intercepted_keys();
    let logo_shift = Modifiers {
        shift: true,
        ..Modifiers::LOGO
    };
    for (modifiers, key) in [
        (Modifiers::LOGO, Key::ArrowLeft),
        (Modifiers::LOGO, Key::D),
        (Modifiers::LOGO, Key::F),
        (Modifiers::ALT, Key::F4),
        (Modifiers::LOGO, Key::Tab),
        (logo_shift, Key::Tab),
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

    tabs_walk_on_ctrl_alt_page_keys();
}

/// With the system keeping Super, the dock's Next tab and Previous tab are Ctrl+Alt+PageDown and
/// Ctrl+Alt+PageUp.
fn tabs_walk_on_ctrl_alt_page_keys() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    fixture.test.set_watched::<SuperKeptWatch>(&true);
    fixture.settle();
    fixture.group_two_windows();
    fixture.settle();
    let ctrl_alt = Modifiers {
        alt: true,
        ..Modifiers::CTRL
    };
    let shown = fixture.shown_tab();
    assert!(fixture.test.host_key(ctrl_alt, Key::PageDown));
    fixture.settle();
    assert_ne!(
        fixture.shown_tab(),
        shown,
        "Ctrl+Alt+PageDown shows the window's next tab"
    );
    assert!(fixture.test.host_key(ctrl_alt, Key::PageUp));
    fixture.settle();
    assert_eq!(fixture.shown_tab(), shown, "and Ctrl+Alt+PageUp walks back");
}
