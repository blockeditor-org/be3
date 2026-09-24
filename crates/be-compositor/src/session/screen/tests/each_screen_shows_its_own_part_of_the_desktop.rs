use super::*;

const RED: [u8; 4] = [0, 0, 255, 255];
const BLUE: [u8; 4] = [255, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

#[test]
fn each_screen_shows_its_own_part_of_the_desktop() {
    let (device, queue) = vulkan_device();
    let gpu = Gpu::new(device.clone(), queue.clone(), FORMAT);
    let arrow = gpu.rgba(
        crate::session::arrow::WIDTH,
        crate::session::arrow::HEIGHT,
        &crate::session::arrow::pixels(),
    );
    let context = Context::new();
    let output = context.run(RawInput::default(), |context| {
        let painter = context.painter();
        painter.rect_filled(
            Rect::from_min_size(pos2(0.0, 0.0), vec2(32.0, 24.0)),
            0.0,
            Color32::from_rgb(255, 0, 0),
        );
        painter.rect_filled(
            Rect::from_min_size(pos2(32.0, 0.0), vec2(32.0, 24.0)),
            0.0,
            Color32::from_rgb(0, 0, 255),
        );
    });
    let mut left = Screen::new(&gpu, (WIDTH, HEIGHT));
    left.rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(32.0, 24.0));
    let mut right = Screen::new(&gpu, (WIDTH, HEIGHT));
    right.rect = Rect::from_min_size(pos2(32.0, 0.0), vec2(32.0, 24.0));
    let frame = Frame {
        output: &output,
        scale: 1.0,
        clear: Color32::BLACK,
        pointer: pos2(40.0, 2.0),
        sprite: &Sprite::Arrow,
        arrow: &arrow,
    };

    let (left_target, right_target) = (target(&gpu), target(&gpu));
    left.draw(&gpu, &frame, &left_target);
    right.draw(&gpu, &frame, &right_target);
    let (left_pixels, right_pixels) = (
        read(&device, &queue, &left_target),
        read(&device, &queue, &right_target),
    );

    assert_eq!(pixel(&left_pixels, 20, 20), RED);
    assert_eq!(pixel(&right_pixels, 20, 20), BLUE);
    assert_eq!(
        pixel(&right_pixels, 10, 10),
        WHITE,
        "the pointer is drawn on the screen it is over"
    );
    assert_eq!(pixel(&left_pixels, 10, 10), RED, "and nowhere else");
    assert!(!left.dirty() && !right.dirty());
}
