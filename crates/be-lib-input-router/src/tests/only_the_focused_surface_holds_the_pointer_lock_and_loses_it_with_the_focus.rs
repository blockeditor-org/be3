use super::*;

#[test]
fn only_the_focused_surface_holds_the_pointer_lock_and_loses_it_with_the_focus() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    assert!(!router.lock(EDITOR), "an unfocused surface is refused");
    router.focus(Some(EDITOR));
    assert!(router.lock(EDITOR));
    assert_eq!(
        targets(&router.route(&test, Input::PointerMotion(be_lib_geometry::vec2(3.0, 0.0)))),
        vec![EDITOR]
    );
    let moved = router.focus(Some(CANVAS));
    assert_eq!(
        moved[0],
        Routed::Input {
            target: EDITOR,
            input: Input::PointerUnlocked,
        }
    );
    assert_eq!(router.locked(), None);
}
