use super::*;
use block_plugin_api::BackPhase;

fn back_events(instances: &mut Instances, offered: bool, event: beui::Event) -> Vec<InputEvent> {
    host::register(
        TARGET,
        Rect::from_min_size(pos2(10.0, 10.0), SIZE),
        Rect::EVERYTHING,
        0,
    );
    if offered {
        host::offer_back(TARGET);
    }
    host::test_frame(vec![event], None, false);
    instances
        .frame_input(PASS, &FrameOverlay::default())
        .into_iter()
        .filter_map(|message| match message {
            Message::Input(batch) => Some(batch.events),
            _ => None,
        })
        .flatten()
        .filter(|event| matches!(event, InputEvent::Back(_) | InputEvent::Key { .. }))
        .collect()
}

#[test]
fn the_back_gesture_goes_to_the_screen_that_handles_it() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);

    let gesture = beui::Event::Back(beui::BackGesture::Invoked);
    let key = beui::Event::Key {
        key: beui::Key::BrowserBack,
        pressed: true,
        repeat: false,
        modifiers: beui::Modifiers::NONE,
    };
    assert!(
        back_events(&mut instances, false, gesture.clone()).is_empty(),
        "a screen with nothing to go back from is not sent the gesture"
    );
    assert_eq!(
        back_events(&mut instances, true, gesture),
        [InputEvent::Back(BackPhase::Invoked)],
        "the screen that said it handles back is sent the gesture"
    );
    assert_eq!(
        back_events(&mut instances, true, key),
        [InputEvent::Back(BackPhase::Invoked)],
        "the back key goes back on that screen too, even without the focus"
    );
}
