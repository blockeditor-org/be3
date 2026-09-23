use super::*;

fn move_to(at: Pos2) {
    host::register(
        TARGET,
        Rect::from_min_size(pos2(10.0, 10.0), SIZE),
        Rect::EVERYTHING,
        0,
    );
    host::test_frame(vec![beui::Event::PointerMoved(at)], Some(at), false);
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
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);

    move_to(pos2(100.0, 50.0));
    let messages = instances.frame_input(PASS, &FrameOverlay::default());
    assert_eq!(
        events(&messages),
        [InputEvent::PointerMoved { x: 90.0, y: 40.0 }]
    );

    move_to(pos2(150.0, 50.0));
    let messages = instances.frame_input(PASS, &FrameOverlay::default());
    assert_eq!(events(&messages), [InputEvent::PointerLeft]);

    move_to(pos2(160.0, 50.0));
    let messages = instances.frame_input(PASS, &FrameOverlay::default());
    assert!(events(&messages).is_empty());
}
