use super::*;

#[test]
fn a_screen_focused_before_the_plugin_has_it_hears_so_with_its_first_input() {
    let mut instances = placed();
    let (messages, _) = instances.forward(
        INSTANCE,
        REGION,
        &forwarded(vec![beui::Forwarded::Focus(true)], None, true),
        &|_, _| beui::Aim::Own,
        &|_| None,
    );
    assert!(
        messages.is_empty(),
        "a screen the plugin does not have yet hears nothing"
    );

    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);
    let typed = beui::Forwarded::Input(beui::Event::Text("a".to_owned()));
    let (messages, _) = instances.forward(
        INSTANCE,
        REGION,
        &forwarded(vec![typed.clone()], None, true),
        &|_, _| beui::Aim::Own,
        &|_| None,
    );
    assert_eq!(
        input_events(&messages),
        [Input::Focus(true), Input::Text("a".to_owned())],
        "the focus it missed comes before its first input"
    );
    let (messages, _) = instances.forward(
        INSTANCE,
        REGION,
        &forwarded(vec![typed], None, true),
        &|_, _| beui::Aim::Own,
        &|_| None,
    );
    assert_eq!(input_events(&messages), [Input::Text("a".to_owned())]);
}
