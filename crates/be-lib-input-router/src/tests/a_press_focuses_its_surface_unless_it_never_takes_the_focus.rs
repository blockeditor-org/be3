use super::*;

#[test]
fn a_press_focuses_its_surface_unless_it_never_takes_the_focus() {
    let mut test = canvas_with_editor().with(3, rect(800.0, 800.0, 100.0, 100.0), Some(ROOT));
    test.last().focus = false;
    let mut router = Router::new(ROOT);
    let pressed = router.route(&test, press(250.0, 250.0, true));
    assert_eq!(
        pressed[0],
        Routed::Input {
            target: EDITOR,
            input: Input::Focus(true),
        }
    );
    router.route(&test, press(250.0, 250.0, false));
    router.route(&test, press(850.0, 850.0, true));
    assert_eq!(
        router.focused(),
        Some(EDITOR),
        "a popup or the on-screen keyboard leaves the focus where it was"
    );
    router.route(&test, press(850.0, 850.0, false));
    let moved = router.focus(Some(CANVAS));
    assert_eq!(
        moved,
        vec![
            Routed::Input {
                target: EDITOR,
                input: Input::Focus(false),
            },
            Routed::Input {
                target: CANVAS,
                input: Input::Focus(true),
            },
        ],
        "the compositor moves the focus, telling both"
    );
}
