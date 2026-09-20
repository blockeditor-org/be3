use super::*;

#[test]
fn resetting_the_artwork_clears_painted_pixels() {
    let (mut editor, block, _host) = editor();
    block.operate(PixelRayTracerOperation::Paint {
        pixels: vec![PixelUpdate {
            x: 4,
            y: 6,
            color_index: 2,
        }],
    });
    editor.run();
    assert!(
        block
            .read()
            .unwrap()
            .pixels()
            .iter()
            .any(|pixel| *pixel != PIXEL_RAY_TRACER_BACKGROUND)
    );

    editor.click("pixel_ray_tracer.reset");
    editor.run();
    editor.run();

    assert!(
        block
            .read()
            .unwrap()
            .pixels()
            .iter()
            .all(|pixel| *pixel == PIXEL_RAY_TRACER_BACKGROUND)
    );
}
