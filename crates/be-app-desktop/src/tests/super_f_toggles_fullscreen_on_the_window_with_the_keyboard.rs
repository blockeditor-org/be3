use super::*;

#[test]
fn super_f_toggles_fullscreen_on_the_window_with_the_keyboard() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    assert!(
        fixture.test.host_key(Modifiers::LOGO, Key::F),
        "the desktop takes Super+F before the program that has the keyboard"
    );
    fixture.settle();
    assert_eq!(
        fixture.fullscreen_windows(),
        vec![HostWindowId(2)],
        "Super+F shows the window with the keyboard full screen"
    );

    let fullscreen = TestWindow {
        fullscreen: Some(Rect::from_min_size(
            Pos2::new(0.0, 0.0),
            Vec2::new(800.0, 600.0),
        )),
        ..host_window(2, true)
    };
    fixture.show_windows(&[host_window(1, false), fullscreen]);
    assert!(fixture.test.host_key(Modifiers::LOGO, Key::F));
    fixture.settle();
    assert!(
        fixture.fullscreen_windows().is_empty(),
        "a second Super+F takes it out again"
    );
}
