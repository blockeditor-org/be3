use super::*;
use block_plugin_api::{Key, Modifiers};

#[test]
fn a_key_intercepted_for_a_plugin_reaches_it_with_its_modifiers() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);
    let alt = beui::Modifiers::ALT;

    let press = beui::KeyPress {
        key: beui::Key::Tab,
        pressed: true,
        repeat: false,
        modifiers: alt,
    };
    let messages = instances.intercepted(INSTANCE, REGION, Some(press), alt);
    assert_eq!(
        input_events(&messages),
        [
            InputEvent::Modifiers(Modifiers {
                alt: true,
                ..Modifiers::default()
            }),
            InputEvent::InterceptedKey {
                key: Key::Tab,
                pressed: true,
                repeat: false,
            },
        ],
        "the press arrives after the modifiers it was made with"
    );

    let messages = instances.intercepted(INSTANCE, REGION, None, alt);
    assert!(
        messages.is_empty(),
        "modifiers the plugin already knows are not sent again"
    );

    let messages = instances.intercepted(INSTANCE, REGION, None, beui::Modifiers::NONE);
    assert_eq!(
        input_events(&messages),
        [InputEvent::Modifiers(Modifiers::default())],
        "letting go of Alt reaches the plugin even though no key came with it"
    );
}
