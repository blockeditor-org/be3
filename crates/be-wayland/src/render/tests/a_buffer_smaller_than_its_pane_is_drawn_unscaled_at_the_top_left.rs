use super::*;

#[test]
fn a_buffer_smaller_than_its_pane_is_drawn_unscaled_at_the_top_left() {
    let at = pane(100.0, 50.0, 400.0, 300.0, 1.0);
    let buffer = surface(300.0, 200.0);
    assert_eq!(
        drawn(buffer, buffer, (300.0, 200.0), &at),
        Drawn {
            pixels: [100.0, 50.0, 400.0, 250.0],
            uv: [0.0, 0.0, 1.0, 1.0],
        },
        "the whole buffer is drawn at its own size from the pane's corner, leaving the rest of the pane"
    );
}
