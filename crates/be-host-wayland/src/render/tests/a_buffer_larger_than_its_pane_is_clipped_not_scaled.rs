use super::*;

#[test]
fn a_buffer_larger_than_its_pane_is_clipped_not_scaled() {
    let at = pane(100.0, 50.0, 400.0, 300.0, 1.0);
    let buffer = surface(500.0, 600.0);
    assert_eq!(
        drawn(buffer, buffer, (500.0, 600.0), &at),
        Drawn {
            pixels: [100.0, 50.0, 500.0, 350.0],
            uv: [0.0, 0.0, 0.8, 0.5],
        },
        "the pane shows the buffer's top-left corner, one buffer pixel to a pixel"
    );
}
