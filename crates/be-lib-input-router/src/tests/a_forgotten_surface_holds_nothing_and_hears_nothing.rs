use super::*;

#[test]
fn a_forgotten_surface_holds_nothing_and_hears_nothing() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, press(250.0, 250.0, true));
    router.route(&test, key(Key::Tab, true));
    assert!(router.lock(EDITOR));
    router.forget(EDITOR);
    assert_eq!(
        (
            router.pressed(),
            router.focused(),
            router.hovered(),
            router.locked()
        ),
        (None, None, None, None)
    );
    assert!(router.route(&test, key(Key::Tab, false)).is_empty());
    assert!(
        router
            .route(&test, Input::PointerMotion(be_lib_geometry::vec2(1.0, 0.0)))
            .is_empty()
    );
    assert_eq!(
        targets(&router.route(&test, Input::PointerMoved(pos2(150.0, 150.0)))),
        vec![CANVAS],
        "the pointer it held is free again"
    );
}
