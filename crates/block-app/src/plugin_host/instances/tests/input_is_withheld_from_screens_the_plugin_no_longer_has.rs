use super::*;
use block_plugin_api::BackGesture;

#[test]
fn input_is_withheld_from_screens_the_plugin_no_longer_has() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    assert_eq!(screens.len(), 1);
    instances.screen_set(screens);

    let (messages, _) = instances.forward(
        INSTANCE,
        REGION,
        &forwarded(vec![beui::Forwarded::Focus(true)], None, true),
        &|_, _| beui::Aim::Own,
        &|_| None,
    );
    assert_eq!(input_events(&messages), [Input::Focus(true)]);
    let messages = instances.back(INSTANCE, REGION, beui::BackGesture::Invoked);
    assert_eq!(input_events(&messages), [Input::Back(BackGesture::Invoked)]);

    instances.screen_set(Vec::new());

    let (messages, _) = instances.forward(
        INSTANCE,
        REGION,
        &forwarded(Vec::new(), None, false),
        &|_, _| beui::Aim::Own,
        &|_| None,
    );
    assert!(messages.is_empty());
    assert!(
        instances
            .back(INSTANCE, REGION, beui::BackGesture::Invoked)
            .is_empty()
    );
}
