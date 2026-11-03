use super::*;

#[test]
fn a_window_drawn_past_its_body_fades_along_the_clipped_edges() {
    assert_eq!(
        clipped_edges(vec2(300.0, 200.0), vec2(400.0, 300.0)),
        Sides::default(),
        "a window drawn within its body fades nowhere"
    );
    let wide = clipped_edges(vec2(500.0, 200.0), vec2(400.0, 300.0));
    assert_eq!((wide.left, wide.top, wide.bottom), (0.0, 0.0, 0.0));
    assert!(wide.right > 0.0, "it fades on the edge it is clipped at");
    let both = clipped_edges(vec2(500.0, 400.0), vec2(400.0, 300.0));
    assert!(both.right > 0.0 && both.bottom > 0.0);

    let mut shown = shown(status(vec2(900.0, 600.0), Vec2::ZERO, true));
    shown
        .stepped
        .snapshot("a_window_drawn_past_its_body_fades_along_the_clipped_edges");
}
