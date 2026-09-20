use super::*;

#[test]
fn a_surface_is_drawn_along_the_line_it_spans() {
    let surface = RayEntity::Surface {
        id: 1,
        start: Point::new(8.0, 8.0),
        end: Point::new(24.0, 24.0),
        color_index: 2,
        roughness: 0.0,
        metalness: 0.0,
        transmission: 0.0,
        refractive_index: 1.5,
    };

    let image = draw(&[surface], None, &Preview::None);

    assert_eq!(
        (image.width(), image.height()),
        (OVERLAY_SIZE, OVERLAY_SIZE)
    );
    assert!(alpha(&image, 16, 16) > 0, "the middle of the line is drawn");
    assert_eq!(alpha(&image, 16, 4), 0, "nothing is drawn beside the line");
    assert_eq!(alpha(&image, 40, 8), 0, "nothing is drawn past the end");
}

fn alpha(image: &block_editor_plugin::beui::Image, x: u32, y: u32) -> u8 {
    let at = ((y * OVERLAY_SCALE * OVERLAY_SIZE + x * OVERLAY_SCALE) * 4 + 3) as usize;
    image.pixels()[at]
}
