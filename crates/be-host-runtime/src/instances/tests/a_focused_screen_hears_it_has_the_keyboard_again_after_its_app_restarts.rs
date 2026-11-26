use super::*;

#[test]
fn a_focused_screen_hears_it_has_the_keyboard_again_after_its_app_restarts() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    instances.announce(screens);
    let (messages, _) = instances.forward(
        INSTANCE,
        REGION,
        &forwarded(vec![beui::Forwarded::Focus(true)], None, true),
        &|_, _| beui::Aim::Own,
        &|_| None,
    );
    assert_eq!(
        input_events(&instances.gate(messages)),
        [Input::Focus(true)]
    );

    restart(&mut instances);
    let screens = instances.next_screens(PASS).screens;
    assert_eq!(
        input_events(&instances.announce(screens)),
        [Input::Focus(true)],
        "the restarted app's runner starts its views without the keyboard, so the focused one \
         hears it has it before any key"
    );
}
