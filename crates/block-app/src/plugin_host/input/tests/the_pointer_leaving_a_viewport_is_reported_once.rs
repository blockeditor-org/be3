use super::*;

#[test]
fn the_pointer_leaving_a_viewport_is_reported_once() {
    let context = egui::Context::default();
    let rect = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(100.0, 100.0));
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

    assert_eq!(
        frame(vec![egui::Event::PointerMoved(egui::pos2(109.0, 50.0))]),
        vec![InputEvent::PointerMoved { x: 99.0, y: 40.0 }]
    );
    assert_eq!(
        frame(vec![egui::Event::PointerMoved(egui::pos2(115.0, 50.0))]),
        vec![InputEvent::PointerLeft]
    );
    assert_eq!(
        frame(vec![egui::Event::PointerMoved(egui::pos2(120.0, 50.0))]),
        Vec::new()
    );
}
