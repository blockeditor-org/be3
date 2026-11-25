use super::*;

#[test]
fn a_modifier_change_for_a_screen_not_announced_yet_is_sent_again_once_it_is() {
    let mut instances = placed();
    let ctrl = beui::Modifiers {
        ctrl: true,
        ..beui::Modifiers::NONE
    };
    let held = |instances: &mut Instances| {
        let (messages, _) = instances.forward(
            INSTANCE,
            REGION,
            &forwarded(
                vec![beui::Forwarded::Input(beui::Event::Modifiers(ctrl))],
                None,
                true,
            ),
            &|_, _| beui::Aim::Own,
            &|_| None,
        );
        messages
    };
    let messages = held(&mut instances);
    assert!(
        instances.gate(messages).is_empty(),
        "a screen the app does not have yet hears nothing"
    );

    let screens = instances.next_screens(PASS).screens;
    instances.announce(screens);

    // The app heard no modifiers while the screen was missing, so the same ones are news.
    let messages = held(&mut instances);
    assert!(
        input_events(&instances.gate(messages)).contains(&Input::Modifiers(ctrl)),
        "the modifiers the dropped input held reach the app with the next input"
    );
}
