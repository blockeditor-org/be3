use super::*;

/// A gesture claim: beui's middle press over a camera's gesture area pans it rather than
/// reaching the app view under it (`HitTest::pans`), even while the primary button holds that
/// view, and the release of the button holding it still reaches it.
#[test]
fn a_middle_press_that_pans_goes_to_no_surface_even_one_the_mouse_holds() {
    let mut test = canvas_with_editor();
    test.pans = Some(rect(100.0, 100.0, 600.0, 600.0));
    let mut router = Router::default();
    let pressed = router.press(
        &test,
        Pointer::Mouse,
        PointerButton::Middle,
        pos2(250.0, 250.0),
    );
    assert_eq!(pressed, None, "the pan is no surface's");
    assert!(router.pressing(Pointer::Mouse));
    assert_eq!(
        router.moved(&test, Pointer::Mouse, pos2(260.0, 260.0)),
        None,
        "nor is the pointer while the pan holds it"
    );
    assert_eq!(router.release(Pointer::Mouse, PointerButton::Middle), None);

    test.pans = None;
    let held = router.press(
        &test,
        Pointer::Mouse,
        PointerButton::Primary,
        pos2(250.0, 250.0),
    );
    assert_eq!(held, Some(EDITOR));
    test.pans = Some(rect(100.0, 100.0, 600.0, 600.0));
    assert_eq!(
        router.press(
            &test,
            Pointer::Mouse,
            PointerButton::Middle,
            pos2(250.0, 250.0)
        ),
        None
    );
    assert_eq!(
        router.release(Pointer::Mouse, PointerButton::Middle),
        Some(EDITOR),
        "the pan's release goes where the primary button holds the pointer"
    );
    assert_eq!(
        router.release(Pointer::Mouse, PointerButton::Primary),
        Some(EDITOR)
    );
    assert!(!router.pressing(Pointer::Mouse));
}
