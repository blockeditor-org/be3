use super::*;

#[test]
fn a_window_shown_a_snapped_pixel_short_grows_by_that_much() {
    let short = snapped_short(vec2(640.0, 480.0), vec2(640.0, 1438.0 / 3.0))
        .expect("a window laid out a pixel short at 1.5 is grown");
    assert!(
        short.x == 0.0 && (short.y - 2.0 / 3.0).abs() < 1e-4,
        "the window grows by what its chrome's snapping took from it: {short:?}"
    );
    assert_eq!(
        snapped_short(vec2(640.0, 480.0), vec2(640.0, 480.0)),
        None,
        "a window shown at its own size is left alone"
    );
    assert_eq!(
        snapped_short(vec2(640.0, 480.0), vec2(800.0, 600.0)),
        None,
        "a window being resized, larger than its program has drawn yet, is not pulled back"
    );
}
