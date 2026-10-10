use super::*;
use block_plugin_api::Key;

#[test]
fn a_tap_intercepted_for_a_plugin_reaches_it_unless_it_has_the_keyboard() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);

    let messages = instances.intercepted_tap(INSTANCE, REGION, beui::Key::Logo);
    assert_eq!(
        input_events(&messages),
        [InputEvent::InterceptedTap { key: Key::Logo }],
        "a tap heard while something else has the keyboard is sent on"
    );

    instances.forward(INSTANCE, REGION, &forwarded(Vec::new(), None, true));
    let messages = instances.intercepted_tap(INSTANCE, REGION, beui::Key::Logo);
    assert!(
        messages.is_empty(),
        "a plugin with the keyboard hears the key itself, so the tap is not sent twice"
    );
}
