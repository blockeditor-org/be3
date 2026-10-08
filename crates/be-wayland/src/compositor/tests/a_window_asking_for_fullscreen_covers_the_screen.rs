use super::*;

#[test]
fn a_window_asking_for_fullscreen_covers_the_screen() {
    let mut harness = Harness::new();
    let (window, id) = harness.open();
    harness.click("test.input");
    assert!(!harness.client.received.activated);

    window.toplevel.as_ref().unwrap().set_fullscreen(None);
    harness.settle();

    let listed = harness.app.windows().list().get_untracked();
    assert_eq!(
        listed[0].fullscreen,
        Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
        "the ui is told the window wants the whole screen"
    );
    assert_eq!(harness.window(id).size(), SCREEN);
    assert_eq!(
        harness.client.received.size,
        Some((SCREEN.x as i32, SCREEN.y as i32)),
        "the client is told the size of the screen"
    );
    assert!(harness.client.received.fullscreen);
    assert!(
        harness.client.received.activated,
        "the fullscreen window takes the keyboard"
    );
    assert_eq!(harness.app.windows().focused(), Some(id));
}
