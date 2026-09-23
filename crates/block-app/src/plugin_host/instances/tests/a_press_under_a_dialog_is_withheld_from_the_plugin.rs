use super::*;

fn press_under_dialog(at: Pos2) {
    host::register(
        TARGET,
        Rect::from_min_size(pos2(10.0, 10.0), SIZE),
        Rect::EVERYTHING,
        0,
    );
    host::test_frame_under_modal(
        vec![
            beui::Event::PointerMoved(at),
            beui::Event::PointerButton {
                pos: at,
                button: beui::PointerButton::Primary,
                pressed: true,
                modifiers: beui::Modifiers::NONE,
            },
        ],
        Some(at),
        true,
    );
}

#[test]
fn a_press_under_a_dialog_is_withheld_from_the_plugin() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);

    press_under_dialog(pos2(50.0, 50.0));
    let messages = instances.frame_input(PASS, &FrameOverlay::default());

    assert!(!messages.iter().any(|message| match message {
        Message::Input(batch) => batch.events.iter().any(|event| matches!(
            event,
            InputEvent::PointerButton { .. } | InputEvent::PointerMoved { .. }
        )),
        _ => false,
    }));
}
