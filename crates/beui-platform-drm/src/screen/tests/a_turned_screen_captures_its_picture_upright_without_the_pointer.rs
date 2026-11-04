use super::*;

#[test]
fn a_turned_screen_captures_its_picture_upright_without_the_pointer() {
    let (device, queue) = vulkan_device();
    let gpu = Gpu::new(device, queue, FORMAT);
    let arrow = gpu.rgba(
        crate::arrow::WIDTH,
        crate::arrow::HEIGHT,
        &crate::arrow::pixels(),
    );
    let context = beui_host::native::context();
    let mut screen = Screen::new(&gpu, (WIDTH, HEIGHT), Rotation::Clockwise90);
    screen.rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(24.0, 32.0));
    let output = context
        .run(|context| {
            let painter = context.painter();
            painter.rect_filled(
                Rect::from_min_size(pos2(0.0, 0.0), vec2(24.0, 32.0)),
                0.0,
                Color32::from_rgb(255, 0, 0),
            );
            painter.rect_filled(
                Rect::from_min_size(pos2(0.0, 0.0), vec2(4.0, 4.0)),
                0.0,
                Color32::from_rgb(0, 0, 255),
            );
        })
        .main()
        .clone();
    screen.prepare(&gpu, &output);
    let frame = Frame {
        clear: Color32::BLACK,
        pointer: pos2(12.0, 20.0),
        sprite: Pointer::Sprite {
            sprite: &arrow,
            size: vec2(crate::arrow::WIDTH as f32, crate::arrow::HEIGHT as f32),
            hotspot: Vec2::ZERO,
        },
    };
    let (commands, _) = screen.draw(&gpu, &frame, &target(&gpu));
    gpu.queue().submit([commands]);

    let capture = screen.capture(&gpu).expect("a drawn screen reads back");

    assert_eq!((capture.width, capture.height), (HEIGHT, WIDTH));
    let at = |x: u32, y: u32| {
        let at = ((y * capture.width + x) * 4) as usize;
        <[u8; 4]>::try_from(&capture.rgba[at..at + 4]).unwrap()
    };
    assert_eq!(at(1, 1), [0, 0, 255, 255], "in RGBA, upright");
    assert_eq!(at(20, 28), [255, 0, 0, 255]);
    assert_eq!(at(13, 21), [255, 0, 0, 255], "the pointer is not in it");
}
