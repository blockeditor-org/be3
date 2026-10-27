use super::*;

#[test]
fn zooming_stops_the_view_following_its_fit_until_fit_is_pressed() {
    let (test, _editor) = editor();
    let mut test = test.in_viewport();
    test.run();
    assert_eq!(shown(&mut test, "pan_zoom.zoom"), "\"Zoom 66%\"");

    test.click("pan_zoom.zoom_in");
    assert_eq!(shown(&mut test, "pan_zoom.zoom"), "\"Zoom 82%\"");

    test.set_size(Vec2::new(1400.0, 900.0));
    assert_eq!(
        shown(&mut test, "pan_zoom.zoom"),
        "\"Zoom 82%\"",
        "a view the person moved stays where they put it when the window grows"
    );

    test.click("pan_zoom.fit");
    assert_eq!(shown(&mut test, "pan_zoom.zoom"), "\"Zoom 100%\"");
    let stage = test.content_rect().expect("the editor reports its stage");
    let origin = test.rect_of("pan_zoom.card.0").min;
    let expected = stage.center() - Vec2::new(440.0, 290.0);
    assert!(
        (origin - expected).length() < 1.0,
        "Fit puts the world back in the middle, the origin card at {origin:?} rather than {expected:?}"
    );
}
