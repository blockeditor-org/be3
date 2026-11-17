use super::*;

/// An app's view (`AppView`): what heard the pointer hears it leave once the pointer is over
/// something else and no press holds it, as beui's catchers are told `Forwarded::Left`.
#[test]
fn an_apps_view_hears_the_pointer_leave_once_no_press_holds_it() {
    let test = canvas_with_editor();
    let mut router = Router::default();
    router.entered(EDITOR);
    assert_eq!(router.hover(Some(EDITOR)), Vec::<u32>::new());
    router.press(&test, Pointer::Mouse, PointerButton::Primary, pos2(250.0, 250.0));
    assert_eq!(
        router.hover(Some(CANVAS)),
        Vec::<u32>::new(),
        "the press holds it while the pointer is elsewhere"
    );
    router.release(Pointer::Mouse, PointerButton::Primary);
    assert_eq!(router.hover(Some(CANVAS)), vec![EDITOR]);
    assert_eq!(router.hovered(), Some(CANVAS));
    let finger = Pointer::Touch(TouchId { device: 0, finger: 1 });
    router.entered(CANVAS);
    router.press(&test, finger, PointerButton::Primary, pos2(150.0, 150.0));
    assert_eq!(
        router.hover(None),
        vec![CANVAS],
        "a finger's press keeps no hover"
    );
}
