use super::*;

#[test]
fn a_scaled_buffer_is_drawn_at_its_logical_size() {
    let at = pane(200.0, 100.0, 800.0, 600.0, 2.0);
    let logical = surface(300.0, 200.0);
    assert_eq!(
        drawn(logical, logical, (300.0, 200.0), &at),
        Drawn {
            pixels: [200.0, 100.0, 800.0, 500.0],
            uv: [0.0, 0.0, 1.0, 1.0],
        },
        "a buffer at scale 2 covers its logical size in points, at two pixels to a point"
    );
}
