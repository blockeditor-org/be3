use super::*;

#[test]
fn the_stage_stays_transparent_so_the_host_canvas_shows_through() {
    let (mut test, _editor) = editor();

    test.set_view(
        Rect::from_min_size(pos2(300.0, 60.0), Vec2::new(440.0, 290.0)),
        0.5,
    );
    test.run();

    test.snapshot("the_stage_stays_transparent_so_the_host_canvas_shows_through");
}
