use super::*;

fn paint(context: &beui::native::Context, square: bool) -> beui::native::WindowOutput {
    context
        .run(|context| {
            let painter = context.painter();
            painter.rect_filled(
                Rect::from_min_size(pos2(0.0, 0.0), vec2(32.0, 24.0)),
                0.0,
                Color32::from_rgb(255, 0, 0),
            );
            if square {
                painter.rect_filled(
                    Rect::from_min_size(pos2(20.0, 10.0), vec2(4.0, 4.0)),
                    0.0,
                    Color32::from_rgb(0, 0, 255),
                );
            }
        })
        .main()
        .clone()
}

#[test]
fn the_display_is_told_only_what_changed() {
    let (device, queue) = vulkan_device();
    let gpu = Gpu::new(device, queue, FORMAT);
    let context = beui_host::native::context();
    let mut screen = Screen::new(&gpu, (WIDTH, HEIGHT), Rotation::Normal);
    screen.rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(32.0, 24.0));
    let target = target(&gpu);
    let frame = Frame {
        clear: Color32::BLACK,
        pointer: pos2(0.0, 0.0),
        sprite: Pointer::Hidden,
    };

    let first = paint(&context, false);
    screen.prepare(&gpu, &first);
    let (commands, damage) = screen.draw(&gpu, &frame, &target);
    gpu.queue().submit([commands]);
    assert_eq!(damage, None, "the first frame changes the whole screen");

    let second = paint(&context, true);
    screen.damage(Repaint::Region {
        region: Rect::from_min_size(pos2(20.0, 10.0), vec2(4.0, 4.0)).into(),
        background: Color32::BLACK,
    });
    screen.prepare(&gpu, &second);
    let (commands, damage) = screen.draw(&gpu, &frame, &target);
    gpu.queue().submit([commands]);
    let damage = damage.expect("a frame that changes a part names that part");
    let square = Rectangle::<i32, Physical>::new((20, 10).into(), (4, 4).into());
    assert!(
        damage.iter().any(|rect| rect.contains_rect(square)),
        "the square is in the damage: {damage:?}"
    );
    assert!(
        damage
            .iter()
            .all(|rect| rect.size.w * rect.size.h < (WIDTH * HEIGHT) as i32 / 2),
        "and little else: {damage:?}"
    );
}
