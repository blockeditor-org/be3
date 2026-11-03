use super::*;

#[test]
fn a_key_intercepted_for_an_app_reaches_it_unless_it_has_the_keyboard() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);
    let press = beui::KeyPress {
        key: beui::Key::F,
        pressed: true,
        repeat: false,
        modifiers: beui::Modifiers::LOGO,
    };

    assert!(!instances.has_keyboard(INSTANCE, REGION));
    let messages = instances.intercepted(INSTANCE, REGION, Some(press), press.modifiers);
    assert!(
        !input_events(&messages).is_empty(),
        "a chord heard while something else has the keyboard is sent on"
    );

    instances.forward(
        INSTANCE,
        REGION,
        &forwarded(vec![beui::Forwarded::Focus(true)], None, true),
        &|_, _| beui::Aim::Own,
        &|_| None,
    );
    assert!(instances.has_keyboard(INSTANCE, REGION));
    let messages = instances.intercepted(INSTANCE, REGION, Some(press), press.modifiers);
    assert!(
        messages.is_empty(),
        "an app with the keyboard hears the chord itself, in order with the Super it was pressed under"
    );
}
