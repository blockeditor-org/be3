use super::*;

fn display(id: &str, rect: beui::Rect, scale: f32) -> crate::display::shown::ShownDisplay {
    crate::display::shown::ShownDisplay {
        id: id.to_owned(),
        name: id.to_owned(),
        rect,
        scale,
        placement: WindowPlacement::Host(HostPlacement(format!("display {id}"))),
    }
}

#[test]
fn each_display_shows_its_own_lock_screen_at_its_own_scale() {
    let laptop = beui::Rect::from_min_size(pos2(0.0, 0.0), vec2(960.0, 600.0));
    let monitor = beui::Rect::from_min_size(pos2(960.0, 120.0), vec2(640.0, 480.0));
    let context = Context::new(FreetypeFonts::default());
    beui::reactive::pin_wall_clock(Some((context.now(), Duration::from_secs(1_700_000_000))));
    let mut document = build(|| {
        crate::compositor::install();
        crate::display::create_handle();
        let view = AppViewStore::new(AppView {
            lock: LockView {
                available: true,
                locked: true,
                user: "Ada Lovelace".to_owned(),
                ..LockView::default()
            },
            ..AppView::default()
        });
        view! {
            <beui::styled::Root title="Block">
                <Displays view />
            </beui::styled::Root>
        }
    });
    crate::display::shown::offer(vec![
        display("laptop", laptop, 2.0),
        display("monitor", monitor, 1.0),
    ]);
    beui::reactive::enter(&mut document, crate::display::apply_shown);
    let window = WindowSize {
        size: vec2(960.0, 600.0),
        scale: 2.0,
    };
    let mut stepped = Stepped::hosting_windows(context, Showing(document), window);
    stepped.frame(Vec::new());
    let opened: Vec<(beui::WindowId, beui::Vec2)> = stepped
        .windows()
        .open
        .iter()
        .map(|window| (window.id, window.size))
        .collect();
    assert_eq!(
        opened.iter().map(|(_, size)| *size).collect::<Vec<_>>(),
        [laptop.size(), monitor.size()],
        "a window opens for each display, at its size"
    );
    for ((window, _), scale) in opened.iter().zip([2.0, 1.0]) {
        stepped.report_window(WindowReport::Rastered {
            window: *window,
            pixels_per_point: scale,
        });
    }
    for _ in 0..3 {
        stepped.advance(Duration::from_millis(300));
        stepped.frame(Vec::new());
    }
    for ((window, size), (name, scale)) in opened.iter().zip([
        ("lock_screen_on_a_doubled_laptop", 2.0),
        ("lock_screen_on_a_monitor", 1.0),
    ]) {
        let windows = stepped.windows();
        let output = windows.output(*window).expect("each display window paints");
        assert_eq!(output.pixels_per_point, scale, "at its display's own scale");
        let painted = beui::painting::painting(
            output,
            *size,
            scale,
            output.window.appearance.clear_color,
            beui::painting::Punches::Shown,
        )
        .unwrap();
        let snapshot = paint_snapshot::Snapshot::of_painting(painted).unwrap();
        paint_snapshot::assert_accepted(name, &snapshot, "./scripts/verify");
    }
}
