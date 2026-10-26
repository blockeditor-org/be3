use super::*;

#[test]
fn a_ray_between_a_window_and_its_chrome_never_falls_through_to_a_window_behind() {
    let (mut space, now) = space();
    let (over, _) = open(
        &mut space,
        now,
        WindowPlacement::Over {
            window: WindowId::MAIN,
            at: pos2(200.0, 150.0),
        },
    );
    if let Some(panel) = space.panels.iter_mut().find(|panel| panel.window == over) {
        panel.chrome.title = 0.06;
    }
    send(&mut space, now, []);
    let panel = *space.panel(over).unwrap();
    let points = |metres: f32| metres * panel.chrome.scale / panel.metres_per_point;
    let reach = points(chrome::GAP + chrome::THICKNESS);
    let size = panel.size;
    let mut outside = Vec::new();
    for step in 0..=40 {
        let t = step as f32 / 40.0;
        for out in [0.2, reach / 2.0, reach - 0.2] {
            outside.push(pos2(size.x * t, -out));
            outside.push(pos2(size.x * t, size.y + out));
            outside.push(pos2(-out, size.y * t));
            outside.push(pos2(size.x + out, size.y * t));
        }
    }
    let metres = panel.metres();
    let layout = chrome::layout(vec2(metres.x, metres.y), panel.chrome, false);
    let label = layout.title.expect("the window's title is shown");
    let in_points = |at: Pos2| pos2(at.x / panel.metres_per_point, at.y / panel.metres_per_point);
    let towards = |rect: Rect, edge: f32, far: f32| {
        (0..=10).flat_map(move |x| {
            (1..=10).map(move |y| {
                let x = rect.min.x + rect.width() * x as f32 / 10.0;
                in_points(pos2(x, edge + (far - edge) * y as f32 / 10.0))
            })
        })
    };
    outside.extend(towards(layout.pill, metres.y, layout.pill.max.y));
    outside.extend(towards(label, 0.0, label.min.y));
    for pos in &outside {
        assert!(
            chrome::part_at(&panel, *pos, false).is_some(),
            "{pos:?}, outside the window and inside its chrome, belongs to a part"
        );
    }

    let under = |pos: Pos2| screen(&space, panel.world(pos));
    let aside = screen(&space, main(&space).world(pos2(60.0, 60.0)));
    let cases = [
        (
            pos2(in_points(layout.buttons[0].1.center()).x, size.y + 0.5),
            Part::Move,
        ),
        (
            pos2(-reach / 2.0, size.y + 0.5),
            Part::Scale(Corner::BottomLeft),
        ),
        (pos2(-0.5, size.y / 2.0), Part::Resize(chrome::Side::Left)),
        (pos2(size.x / 2.0, -0.5), Part::Title),
    ];
    let cases = cases.map(|(pos, part)| (pos, part, under(pos)));
    for (pos, part, at) in cases {
        let sent = send(
            &mut space,
            now,
            [Input::PointerMoved(aside), Input::PointerMoved(at)],
        );
        let got = delivered(&sent);
        assert_eq!(
            got.last(),
            Some(&(WindowId::MAIN, Input::PointerLeft)),
            "the main window behind {pos:?} only sees the cursor leave: {got:?}"
        );
        assert_eq!(space.chrome.hovered, Some((over, part)), "at {pos:?}");
    }
}
