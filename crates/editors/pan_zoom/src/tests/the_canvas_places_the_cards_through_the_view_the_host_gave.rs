use super::*;

#[test]
fn the_canvas_places_the_cards_through_the_view_the_host_gave() {
    let (mut editor, host) = editor();

    host.set_beui_view(
        Rect::from_min_size(pos2(300.0, 60.0), Vec2::new(440.0, 290.0)),
        0.5,
    );
    editor.run();

    assert_eq!(
        editor.rect_of("pan_zoom.card.0"),
        Rect::from_min_size(pos2(300.0, 60.0), Vec2::new(105.0, 55.0))
    );
    assert_eq!(
        editor.rect_of("pan_zoom.card.1"),
        Rect::from_min_size(pos2(430.0, 80.0), Vec2::new(110.0, 60.0))
    );
}
