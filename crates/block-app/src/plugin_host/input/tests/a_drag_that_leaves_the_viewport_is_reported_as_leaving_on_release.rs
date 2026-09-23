use super::*;

#[test]
fn a_drag_that_leaves_the_viewport_is_reported_as_leaving_on_release() {
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
            .filter(|event| !matches!(event, InputEvent::Modifiers(_)))
            .collect::<Vec<_>>()
    };
    let button = |pos: egui::Pos2, pressed: bool| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };

    frame(vec![
        egui::Event::PointerMoved(egui::pos2(50.0, 50.0)),
        button(egui::pos2(50.0, 50.0), true),
    ]);
    assert_eq!(
        frame(vec![egui::Event::PointerMoved(egui::pos2(150.0, 50.0))]),
        vec![InputEvent::PointerMoved { x: 150.0, y: 50.0 }]
    );
    assert_eq!(
        frame(vec![button(egui::pos2(150.0, 50.0), false)]),
        vec![
            InputEvent::PointerButton {
                button: PointerButton::Primary,
                pressed: false,
                x: 150.0,
                y: 50.0,
            },
            InputEvent::PointerLeft,
        ]
    );
}
