use super::*;

fn move_to(context: &egui::Context, id: egui::Id, at: egui::Pos2) {
    let rect = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), SIZE);
    let input = egui::RawInput {
        events: vec![egui::Event::PointerMoved(at)],
        ..egui::RawInput::default()
    };
    let _ = context.run_ui(input, |ui| {
        ui.interact(rect, id, egui::Sense::click_and_drag());
    });
}

fn events(messages: &[Message]) -> Vec<InputEvent> {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::Input(batch) => Some(batch.events.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn a_plugin_is_told_when_the_pointer_leaves_it() {
    let (mut instances, context, id) = placed();
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);

    move_to(&context, id, egui::pos2(100.0, 50.0));
    let messages = instances.frame_input(&context, PASS, &FrameOverlay::default());
    assert_eq!(
        events(&messages),
        [InputEvent::PointerMoved { x: 90.0, y: 40.0 }]
    );

    move_to(&context, id, egui::pos2(150.0, 50.0));
    let messages = instances.frame_input(&context, PASS, &FrameOverlay::default());
    assert_eq!(events(&messages), [InputEvent::PointerLeft]);

    move_to(&context, id, egui::pos2(160.0, 50.0));
    let messages = instances.frame_input(&context, PASS, &FrameOverlay::default());
    assert!(events(&messages).is_empty());
}
