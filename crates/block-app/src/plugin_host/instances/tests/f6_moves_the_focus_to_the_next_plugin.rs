use super::*;

const OTHER: EditorInstanceId = EditorInstanceId(2);

fn press_f6(context: &egui::Context, ids: [egui::Id; 2], modifiers: egui::Modifiers) {
    let input = egui::RawInput {
        events: vec![egui::Event::Key {
            key: egui::Key::F6,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        modifiers,
        ..egui::RawInput::default()
    };
    let _ = context.run_ui(input, |ui| {
        for (id, top) in ids.into_iter().zip([10.0, 120.0]) {
            let rect = egui::Rect::from_min_size(egui::pos2(10.0, top), SIZE);
            ui.interact(rect, id, egui::Sense::click_and_drag());
        }
    });
}

#[test]
fn f6_moves_the_focus_to_the_next_plugin() {
    let (mut instances, context, first) = placed();
    let second = egui::Id::new("second plugin screen");
    let below = egui::Rect::from_min_size(egui::pos2(10.0, 120.0), SIZE);
    let client = Arc::new(BlockClient::new(Uuid::nil(), Uuid::nil()));
    instances.report(
        OTHER,
        REGION,
        &context,
        &client,
        Uuid::nil(),
        InstanceRole::Editor(EditorBlock {
            id: Uuid::nil(),
            block_type: Uuid::nil(),
        }),
        &Arc::new(Vec::new()),
        Some(block_plugin_api::FrameSpec::default()),
        SIZE,
        egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE),
        1.0,
        PASS,
    );
    instances.place(
        OTHER,
        REGION,
        Placement {
            id: second,
            rect: below,
            clip: below,
            pass: PASS,
        },
    );
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);
    context.memory_mut(|memory| memory.request_focus(first));

    press_f6(&context, [first, second], egui::Modifiers::NONE);
    let _ = instances.frame_input(&context, PASS, &FrameOverlay::default());
    assert!(context.memory(|memory| memory.has_focus(second)));

    press_f6(&context, [first, second], egui::Modifiers::NONE);
    let _ = instances.frame_input(&context, PASS, &FrameOverlay::default());
    assert!(context.memory(|memory| memory.has_focus(first)));

    press_f6(&context, [first, second], egui::Modifiers::SHIFT);
    let messages = instances.frame_input(&context, PASS, &FrameOverlay::default());
    assert!(context.memory(|memory| memory.has_focus(second)));
    assert!(!messages.iter().any(|message| match message {
        Message::Input(batch) => batch.events.iter().any(|event| matches!(
            event,
            InputEvent::Key {
                key: block_plugin_api::Key::F6,
                ..
            }
        )),
        _ => false,
    }));
}
