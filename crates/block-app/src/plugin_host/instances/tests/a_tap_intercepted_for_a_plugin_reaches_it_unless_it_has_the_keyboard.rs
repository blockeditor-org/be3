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
        [Input::InterceptedTap(Key::Logo)],
        "a tap heard while something else has the keyboard is sent on"
    );

    instances.forward(
        INSTANCE,
        REGION,
        &forwarded(vec![beui::Forwarded::Focus(true)], None, true),
        &|_, _| beui::Aim::Own,
        &|_| None,
    );
    let messages = instances.intercepted_tap(INSTANCE, REGION, beui::Key::Logo);
    assert!(
        messages.is_empty(),
        "a plugin with the keyboard hears the key itself, so the tap is not sent twice"
    );
}
