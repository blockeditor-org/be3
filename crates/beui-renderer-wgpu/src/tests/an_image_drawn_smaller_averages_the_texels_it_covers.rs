use super::*;

#[test]
fn an_image_drawn_smaller_averages_the_texels_it_covers() {
    const SIDE: u32 = 64;
    let checks: Vec<u8> = (0..SIDE * SIDE)
        .flat_map(|at| match (at % SIDE + at / SIDE) % 2 {
            0 => [255, 255, 255, 255],
            _ => [0, 0, 0, 255],
        })
        .collect();
    let image = Image::from_rgba(SIDE, SIDE, checks);
    let capture = capture_in(
        wgpu::TextureFormat::Rgba8Unorm,
        Color32::BLACK,
        |painter| {
            painter.image(
                Rect::from_min_max(Pos2::ZERO, pos2(8.0, 8.0)),
                Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                &image,
                Color32::WHITE,
                0.0,
                true,
            );
        },
    );
    for y in 1..7 {
        for x in 1..7 {
            let red = capture.pixel(x, y)[0];
            assert!(
                red.abs_diff(128) <= 4,
                "a checkerboard eight times too fine to show draws as its grey, not as a few of its texels: {red} at {x},{y}"
            );
        }
    }
}
