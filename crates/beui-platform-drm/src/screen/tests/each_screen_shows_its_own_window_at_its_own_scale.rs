use super::*;

const RED: [u8; 4] = [0, 0, 255, 255];
const BLUE: [u8; 4] = [255, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn filled(size: Vec2, color: Color32) -> beui::native::WindowOutput {
    let context = beui::native::context();
    let output = context.run(|context| {
        context.painter().rect_filled(
            Rect::from_min_size(pos2(0.0, 0.0), vec2(size.x / 2.0, size.y)),
            0.0,
            color,
        );
    });
    output.main().clone()
}

#[test]
fn each_screen_shows_its_own_window_at_its_own_scale() {
    let (device, queue) = vulkan_device();
    let gpu = Gpu::new(device.clone(), queue.clone(), FORMAT);
    let arrow = gpu.rgba(
        crate::arrow::WIDTH,
        crate::arrow::HEIGHT,
        &crate::arrow::pixels(),
    );
    let mut left = Screen::new(&gpu, (WIDTH, HEIGHT), Rotation::Normal);
    left.rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(32.0, 24.0));
    let mut right = Screen::new(&gpu, (WIDTH, HEIGHT), Rotation::Normal);
    right.scale = 2.0;
    right.rect = Rect::from_min_size(pos2(32.0, 0.0), right.size());
    assert_eq!(
        right.size(),
        vec2(16.0, 12.0),
        "a doubled screen is half as many points"
    );
    left.prepare(&gpu, &filled(left.size(), Color32::from_rgb(255, 0, 0)));
    right.prepare(&gpu, &filled(right.size(), Color32::from_rgb(0, 0, 255)));
    let frame = Frame {
        clear: Color32::BLACK,
        pointer: pos2(42.0, 1.0),
        sprite: Pointer::Sprite {
            sprite: &arrow,
            size: vec2(crate::arrow::WIDTH as f32, crate::arrow::HEIGHT as f32),
            hotspot: Vec2::ZERO,
        },
    };

    let (left_target, right_target) = (target(&gpu), target(&gpu));
    let drawn = [
        left.draw(&gpu, &frame, &left_target).0,
        right.draw(&gpu, &frame, &right_target).0,
    ];
    gpu.queue().submit(drawn);
    let (left_pixels, right_pixels) = (
        read(&device, &queue, &left_target),
        read(&device, &queue, &right_target),
    );

    assert_eq!(pixel(&left_pixels, 10, 20), RED);
    assert_eq!(
        pixel(&left_pixels, 20, 20),
        [0, 0, 0, 255],
        "its window's right half is empty"
    );
    assert_eq!(
        pixel(&right_pixels, 12, 20),
        BLUE,
        "the doubled window's left half covers half the panel"
    );
    assert_eq!(pixel(&right_pixels, 20, 20), [0, 0, 0, 255]);
    assert_eq!(
        pixel(&right_pixels, 24, 18),
        WHITE,
        "the pointer is drawn on the screen it is over, at that screen's scale"
    );
    assert_eq!(pixel(&left_pixels, 10, 3), RED, "and nowhere else");
    assert!(!left.dirty() && !right.dirty());
}
