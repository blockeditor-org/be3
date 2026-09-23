use super::*;

#[test]
fn the_pointer_leaving_the_window_is_reported() {
    let context = egui::Context::default();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 100.0));
    let mut adapter = InputAdapter::default();
    let mut frame = |events: Vec<egui::Event>| {
        let mut output = Vec::new();
        let raw = egui::RawInput {
            events,
            ..Default::default()
        };
        let _ = context.run_ui(raw, |ui| {
            output = adapter.update(ui.ctx(), rect, false, false, ScreenId(0), &Holes::default());
        });
        output
            .into_iter()
            .flat_map(|message| match message {
                Message::Input(batch) => batch.events,
                _ => Vec::new(),
            })
            .collect::<Vec<_>>()
    };

    frame(vec![egui::Event::PointerMoved(egui::pos2(99.0, 50.0))]);
    assert_eq!(
        frame(vec![egui::Event::PointerGone]),
        vec![InputEvent::PointerLeft]
    );
}
