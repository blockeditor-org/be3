use super::*;

#[test]
fn a_screen_announced_while_a_modifier_is_held_is_told_it_as_state() {
    let mut instances = placed();
    let ctrl = beui::Modifiers {
        ctrl: true,
        ..beui::Modifiers::NONE
    };
    let mut held = forwarded(vec![beui::Forwarded::Focus(true)], None, true);
    held.modifiers = ctrl;
    let (messages, _) =
        instances.forward(INSTANCE, REGION, &held, &|_, _| beui::Aim::Own, &|_| None);
    assert!(instances.gate(messages).is_empty());

    let screens = instances.next_screens(PASS).screens;
    let announced = instances.announce(screens);
    assert_eq!(
        input_events(&announced),
        [Input::Focus(true), Input::Modifiers(ctrl)],
        "the screen hears the modifiers are now Ctrl, never a press of it"
    );
}
