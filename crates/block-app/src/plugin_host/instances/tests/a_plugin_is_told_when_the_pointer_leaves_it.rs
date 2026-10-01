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
        &forwarded(vec![beui::Event::PointerMoved(inside)], Some(inside), false),
    );
    assert_eq!(
        input_events(&messages),
        [InputEvent::PointerMoved { x: 90.0, y: 40.0 }]
    );

    let (messages, _) = instances.forward(INSTANCE, REGION, &forwarded(Vec::new(), None, false));
    assert_eq!(input_events(&messages), [InputEvent::PointerLeft]);

    let (messages, _) = instances.forward(INSTANCE, REGION, &forwarded(Vec::new(), None, false));
    assert!(input_events(&messages).is_empty());
}
