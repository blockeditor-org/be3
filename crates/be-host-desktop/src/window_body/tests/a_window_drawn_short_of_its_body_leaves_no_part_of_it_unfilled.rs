use super::*;

#[test]
fn a_window_drawn_short_of_its_body_leaves_no_part_of_it_unfilled() {
    assert_eq!(
        undrawn_from(vec2(300.0, 200.0), vec2(400.0, 300.0)),
        vec2(300.0, 200.0),
        "past a buffer smaller than the body, the body is filled"
    );
    let overhung = undrawn_from(vec2(500.0, 200.0), vec2(400.0, 300.0));
    assert!(
        overhung.x < 400.0 && overhung.y == 200.0,
        "under the fade of an overhanging buffer, the body is filled"
    );

    let drawn = vec2(400.0, 250.0);
    let mut shown = shown_drawing(status(drawn, Vec2::ZERO, true), drawn);
    shown
        .stepped
        .snapshot("a_window_drawn_short_of_its_body_leaves_no_part_of_it_unfilled");
}
