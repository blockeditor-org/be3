use super::*;

#[test]
fn a_super_chord_does_not_open_the_apps_panel() {
    let mut fixture = Fixture::with_windows(&[1]);
    let was = fixture.window_rect(1).expect("window 1 is drawn");
    fixture.test.key_down(Key::Logo);
    fixture.test.key(Key::ArrowLeft, Modifiers::LOGO);
    fixture.test.key_up(Key::Logo);
    fixture.settle();
    assert_ne!(
        fixture.window_rect(1),
        Some(was),
        "Super+Left snaps the window"
    );
    assert!(
        !fixture.test.shown("desktop.apps.panel"),
        "and Super let go after it is no tap, so the Apps panel stays shut"
    );
}
