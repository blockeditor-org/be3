use super::*;

#[test]
fn a_view_moved_before_the_editor_opened_stays_where_it_was() {
    let (test, _editor) = editor();
    let mut test = test.in_moved_viewport(1.5, Vec2::new(30.0, -20.0));
    test.run();

    assert_eq!(
        shown(&mut test, "pan_zoom.zoom"),
        "\"Zoom 150%\"",
        "a block reopened on a view the person moved keeps it rather than fitting"
    );

    test.click("pan_zoom.fit");
    assert_eq!(shown(&mut test, "pan_zoom.zoom"), "\"Zoom 66%\"");
}
