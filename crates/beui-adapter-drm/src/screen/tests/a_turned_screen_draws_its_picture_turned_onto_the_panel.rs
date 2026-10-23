use super::*;

const RED: [u8; 4] = [0, 0, 255, 255];
const BLUE: [u8; 4] = [255, 0, 0, 255];

fn paint(context: &beui::Context, square: bool) -> beui::WindowOutput {
    context
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
            if square {
                painter.rect_filled(
                    Rect::from_min_size(pos2(16.0, 10.0), vec2(4.0, 4.0)),
                    0.0,
                    Color32::from_rgb(0, 0, 255),
                );
            }
        })
        .main()
        .clone()
}

#[test]
fn a_turned_screen_draws_its_picture_turned_onto_the_panel() {
    let (device, queue) = vulkan_device();
    let gpu = Gpu::new(device.clone(), queue.clone(), FORMAT);
    let context = beui::context();
    let mut screen = Screen::new(&gpu, (WIDTH, HEIGHT), Rotation::Clockwise90);
    assert_eq!(
        screen.picture(),
        (HEIGHT, WIDTH),
        "the picture stands upright"
    );
    screen.rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(24.0, 32.0));
    let target = target(&gpu);
    let frame = Frame {
        scale: 1.0,
        clear: Color32::BLACK,
        pointer: pos2(0.0, 0.0),
        sprite: Pointer::Hidden,
    };

    let first = paint(&context, false);
    screen.prepare(&gpu, &first, 1.0);
    let (commands, damage) = screen.draw(&gpu, &frame, &target);
    gpu.queue().submit([commands]);
    assert_eq!(damage, None);
    let pixels = read(&device, &queue, &target);
    assert_eq!(
        pixel(&pixels, WIDTH - 2, 1),
        BLUE,
        "the picture's top left corner is at the panel's top right"
    );
    assert_eq!(pixel(&pixels, 1, 1), RED);
    assert_eq!(pixel(&pixels, WIDTH - 2, HEIGHT - 2), RED);

    let second = paint(&context, true);
    screen.damage(Repaint::Region {
        region: Rect::from_min_size(pos2(16.0, 10.0), vec2(4.0, 4.0)).into(),
        background: Color32::BLACK,
    });
    screen.prepare(&gpu, &second, 1.0);
    let (commands, damage) = screen.draw(&gpu, &frame, &target);
    gpu.queue().submit([commands]);
    let damage = damage.expect("a frame that changes a part names that part");
    let square = Rectangle::<i32, Physical>::new((18, 16).into(), (4, 4).into());
    assert!(
        damage.iter().any(|rect| rect.contains_rect(square)),
        "the damage names where the square lands on the panel: {damage:?}"
    );
    let pixels = read(&device, &queue, &target);
    assert_eq!(pixel(&pixels, 19, 17), BLUE);
}
