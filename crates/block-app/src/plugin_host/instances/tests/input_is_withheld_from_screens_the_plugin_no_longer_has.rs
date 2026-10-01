use super::*;
use block_plugin_api::BackPhase;

#[test]
fn input_is_withheld_from_screens_the_plugin_no_longer_has() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    assert_eq!(screens.len(), 1);
    instances.screen_set(screens);

    let (messages, _) = instances.forward(INSTANCE, REGION, &forwarded(Vec::new(), None, true));
    assert_eq!(input_events(&messages), [InputEvent::Focus(true)]);
    let messages = instances.back(INSTANCE, REGION, beui::BackGesture::Invoked);
    assert_eq!(
        input_events(&messages),
        [InputEvent::Back(BackPhase::Invoked)]
    );

    instances.screen_set(Vec::new());

    let (messages, _) = instances.forward(INSTANCE, REGION, &forwarded(Vec::new(), None, false));
    assert!(messages.is_empty());
    assert!(
        instances
            .back(INSTANCE, REGION, beui::BackGesture::Invoked)
            .is_empty()
    );
}
