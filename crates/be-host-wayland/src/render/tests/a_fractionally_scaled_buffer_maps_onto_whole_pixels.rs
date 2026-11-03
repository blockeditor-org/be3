use super::*;

#[test]
fn a_fractionally_scaled_buffer_maps_onto_whole_pixels() {
    let at = pane(100.4, 50.6, 960.0, 720.0, 1.5);
    let quad = place(
        surface(640.0, 480.0),
        surface(960.0, 720.0),
        (960.0, 720.0).into(),
        &at,
    );
    assert_eq!(
        quad.rect,
        [100.0, 51.0, 1060.0, 771.0],
        "a 960x720 buffer drawn for a 640x480 surface at 1.5 covers exactly 960x720 whole pixels, one texel each"
    );
    let child = place(
        Rectangle::new((10.0, 7.0).into(), (100.0, 33.0).into()),
        surface(150.0, 50.0),
        (150.0, 50.0).into(),
        &at,
    );
    let [left, top, right, bottom] = child.rect;
    assert_eq!(
        [left.fract(), top.fract(), right - left, bottom - top],
        [0.0, 0.0, 150.0, 50.0],
        "a sub-surface at a position that falls between pixels still starts on one, at its buffer's size"
    );
}
