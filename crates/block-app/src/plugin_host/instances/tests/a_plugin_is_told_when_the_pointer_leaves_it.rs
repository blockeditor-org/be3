use super::*;

#[test]
fn a_plugin_is_told_when_the_pointer_leaves_it() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);

    let inside = pos2(100.0, 50.0);
    let (messages, _) = instances.forward(
        INSTANCE,
        REGION,
        &forwarded(vec![moved(inside)], Some(inside), false),
    );
    assert_eq!(
        input_events(&messages),
        [Input::PointerMoved(pos2(90.0, 40.0))]
    );

    let left = forwarded(vec![beui::Forwarded::Left], None, false);
    let (messages, _) = instances.forward(INSTANCE, REGION, &left);
    assert_eq!(input_events(&messages), [Input::PointerLeft]);
}
