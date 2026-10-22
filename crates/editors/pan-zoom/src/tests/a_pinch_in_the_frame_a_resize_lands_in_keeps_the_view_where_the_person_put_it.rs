use super::*;

#[test]
fn a_pinch_in_the_frame_a_resize_lands_in_keeps_the_view_where_the_person_put_it() {
    let (test, _editor) = editor();
    let mut test = test.in_viewport();
    test.run();
    assert_eq!(shown(&mut test, "pan_zoom.zoom"), "\"Zoom 66%\"");

    test.move_view(ViewChange::Zoom {
        factor: 1.5,
        anchor: None,
    });
    test.set_size(Vec2::new(1400.0, 900.0));
    assert_eq!(
        shown(&mut test, "pan_zoom.zoom"),
        "\"Zoom 99%\"",
        "the content band changed in the frame the pinch landed in, and the pinch wins"
    );

    test.set_size(Vec2::new(900.0, 700.0));
    assert_eq!(
        shown(&mut test, "pan_zoom.zoom"),
        "\"Zoom 99%\"",
        "and the view no longer follows the fit after it"
    );
}
