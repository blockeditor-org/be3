use super::*;

#[test]
fn a_wheel_over_an_editor_goes_to_the_outermost_camera_it_grants_the_gesture() {
    let mut test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, Input::PointerMoved(pos2(250.0, 250.0)));
    let scrolled =
        |router: &mut Router<u32>, test: &Rects| targets(&router.route(test, Input::Zoom(1.1)));
    assert_eq!(scrolled(&mut router, &test), vec![EDITOR]);
    test.surfaces[2].gestures = Some(rect(200.0, 200.0, 200.0, 200.0));
    assert_eq!(scrolled(&mut router, &test), vec![CANVAS]);
    test.surfaces[1].gestures = Some(rect(100.0, 100.0, 600.0, 600.0));
    assert_eq!(
        scrolled(&mut router, &test),
        vec![ROOT],
        "claims go outermost first"
    );
}
