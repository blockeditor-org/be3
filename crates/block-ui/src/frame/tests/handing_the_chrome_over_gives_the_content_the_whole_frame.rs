use super::*;

#[test]
fn handing_the_chrome_over_gives_the_content_the_whole_frame() {
    let size = egui::vec2(1200.0, 800.0);
    let context = egui::Context::default();
    let mut drawn = FrameOutcome::default();
    let mut handed = FrameOutcome::default();
    for pass in 0..4 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..egui::RawInput::default()
        };
        let mut bands = Bands::default();
        let _ = context.run_ui(input, |ui| {
            let chrome = match pass < 2 {
                true => Chrome::Drawn,
                false => Chrome::None,
            };
            let outcome = frame().chrome(chrome).show(ui, &mut bands);
            match pass < 2 {
                true => drawn = outcome,
                false => handed = outcome,
            }
        });
    }
    assert!(drawn.rects.toolbar.is_some());
    assert!(drawn.rects.left_sidebar.is_some());
    assert!(drawn.rects.right_sidebar.is_some());
    assert!(drawn.rects.content_band.area() < handed.rects.content_band.area());
    assert_eq!(handed.rects.content_band, handed.rects.frame);
    assert_eq!(handed.rects.toolbar, None);
    assert_eq!(handed.rects.left_sidebar, None);
    assert_eq!(handed.rects.right_sidebar, None);
}
