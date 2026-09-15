use super::*;

#[test]
fn the_stage_stays_transparent_so_the_host_canvas_shows_through() {
    let (mut editor, host) = editor();

    host.set_beui_view(
        Rect::from_min_size(pos2(300.0, 60.0), Vec2::new(440.0, 290.0)),
        0.5,
    );
    editor.run();

    editor.snapshot("the_stage_stays_transparent_so_the_host_canvas_shows_through");
}
