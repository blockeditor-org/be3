use super::*;

/// A win32 popup's content (`press_focus=false`) or a Wayland popup: a press on it leaves the
/// focus with the window it belongs to, which hears no focus change.
#[test]
fn a_window_that_never_takes_the_focus_leaves_it_with_the_window_that_had_it() {
    const POPUP: u32 = 3;
    let mut test = canvas_with_editor().with(POPUP, rect(300.0, 300.0, 100.0, 100.0), Some(EDITOR));
    test.last().focus = false;
    let mut router = Router::new(ROOT);
    router.route(&test, press(250.0, 250.0, true));
    router.route(&test, press(250.0, 250.0, false));
    let pressed = router.route(&test, press(350.0, 350.0, true));
    assert!(
        pressed
            .iter()
            .all(|routed| !matches!(routed, Routed::Input { input: Input::Focus(_), .. })),
        "no one hears the focus move: {pressed:?}"
    );
    assert_eq!(router.focused(), Some(EDITOR));
}
