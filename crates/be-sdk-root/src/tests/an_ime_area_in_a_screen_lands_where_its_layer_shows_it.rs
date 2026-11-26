use super::*;

#[test]
fn an_ime_area_in_a_screen_lands_where_its_layer_shows_it() {
    // The screen is painted at 2 pixels a point and shown a point to a point, at (40, 20).
    let layer = shown_at((40.0, 20.0), 2.0);
    let field = Rect::from_min_max(pos2(10.0, 30.0), pos2(110.0, 50.0));
    let caret = Rect::from_min_max(pos2(12.0, 30.0), pos2(13.0, 50.0));
    let placed = ime_in_window(area(field, caret), 2.0, &layer);
    assert_eq!(
        placed,
        area(
            Rect::from_min_max(pos2(50.0, 50.0), pos2(150.0, 70.0)),
            Rect::from_min_max(pos2(52.0, 50.0), pos2(53.0, 70.0)),
        ),
        "the field and caret keep their size in points and move by where the screen is shown"
    );

    // Painted at 2 a point but shown with a pixel to a point, as a zoomed screen is.
    let mut zoomed = layer;
    zoomed.transform = Affine::from_axes(vec2(1.0, 0.0), vec2(0.0, 1.0), pos2(40.0, 20.0));
    let placed = ime_in_window(area(field, caret), 2.0, &zoomed);
    assert_eq!(
        placed.rect,
        Rect::from_min_max(pos2(60.0, 80.0), pos2(260.0, 120.0)),
        "the area scales with the layer"
    );
}
