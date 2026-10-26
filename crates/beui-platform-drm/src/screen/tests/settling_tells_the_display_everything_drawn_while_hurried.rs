use super::*;

fn paint(context: &beui::Context, squares: &[Rect]) -> beui::WindowOutput {
    context
        .run(|context| {
            let painter = context.painter();
            painter.rect_filled(
                Rect::from_min_size(pos2(0.0, 0.0), vec2(32.0, 24.0)),
                0.0,
                Color32::from_rgb(255, 0, 0),
            );
            for square in squares {
                painter.rect_filled(*square, 0.0, Color32::from_rgb(0, 0, 255));
            }
        })
        .main()
        .clone()
}

#[test]
fn settling_tells_the_display_everything_drawn_while_hurried() {
    let (device, queue) = vulkan_device();
    let gpu = Gpu::new(device, queue, FORMAT);
    let context = beui::context();
    let mut screen = Screen::new(&gpu, (WIDTH, HEIGHT), Rotation::Normal);
    screen.rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(32.0, 24.0));
    let target = target(&gpu);
    let frame = Frame {
        clear: Color32::BLACK,
        pointer: pos2(0.0, 0.0),
        sprite: Pointer::Hidden,
    };
    let draw = |screen: &mut Screen, squares: &[Rect], changed: Option<Rect>| {
        let output = paint(&context, squares);
        if let Some(changed) = changed {
            screen.damage(Repaint::Region {
                region: changed.into(),
                background: Color32::BLACK,
            });
        }
        screen.prepare(&gpu, &output);
        let (commands, damage) = screen.draw(&gpu, &frame, &target);
        gpu.queue().submit([commands]);
        damage
    };
    let left = Rect::from_min_size(pos2(2.0, 2.0), vec2(4.0, 4.0));
    let right = Rect::from_min_size(pos2(20.0, 10.0), vec2(4.0, 4.0));

    draw(&mut screen, &[], None);
    draw(&mut screen, &[], None);
    screen.hurry();
    draw(&mut screen, &[left], Some(left));
    draw(&mut screen, &[left, right], Some(right));
    screen.settle();
    assert!(screen.dirty(), "a settling screen asks for a frame");

    let damage = draw(&mut screen, &[left, right], None)
        .expect("settling names a part of the screen, not all of it");
    let both = Rectangle::<i32, Physical>::new((2, 2).into(), (22, 12).into());
    assert!(
        damage.iter().any(|rect| rect.contains_rect(both)),
        "both squares drawn while hurried are in the damage: {damage:?}"
    );
    assert!(
        damage
            .iter()
            .all(|rect| rect.size.w * rect.size.h < (WIDTH * HEIGHT) as i32),
        "and not the whole screen: {damage:?}"
    );
    assert_eq!(
        draw(&mut screen, &[left, right], None),
        Some(Vec::new()),
        "the next frame is back to naming only what changed"
    );
}
