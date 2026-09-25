use super::*;

const OTHER: EditorInstanceId = EditorInstanceId(2);

fn press_f6(modifiers: beui::Modifiers) {
    host::test_frame(
        vec![beui::Event::Key {
            key: beui::Key::F6,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        None,
        false,
    );
}

#[test]
fn f6_moves_the_focus_to_the_next_plugin() {
    let mut instances = placed();
    let second = Target {
        instance: OTHER,
        region: REGION,
    };
    let below = Rect::from_min_size(pos2(10.0, 120.0), SIZE);
    instances.report(
        OTHER,
        REGION,
        Uuid::nil(),
        InstanceRole::Editor(EditorBlock {
            id: Uuid::nil(),
            block_type: Uuid::nil(),
        }),
        &Arc::new(Vec::new()),
        Some(block_plugin_api::FrameSpec::default()),
        SIZE,
        Rect::from_min_size(Pos2::ZERO, SIZE),
        1.0,
        PASS,
    );
    instances.place(
        OTHER,
        REGION,
        Placement {
            target: second,
            rect: below,
            clip: below,
            pass: PASS,
        },
    );
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);
    host::request_focus(TARGET);

    press_f6(beui::Modifiers::NONE);
    let _ = instances.frame_input(PASS, &FrameOverlay::default());
    assert!(host::focused(second));

    press_f6(beui::Modifiers::NONE);
    let _ = instances.frame_input(PASS, &FrameOverlay::default());
    assert!(host::focused(TARGET));

    press_f6(beui::Modifiers::SHIFT);
    let messages = instances.frame_input(PASS, &FrameOverlay::default());
    assert!(host::focused(second));
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
