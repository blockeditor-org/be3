use super::*;

#[test]
fn input_is_withheld_from_screens_the_plugin_no_longer_has() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    assert_eq!(screens.len(), 1);
    instances.screen_set(screens);
    host::request_focus(TARGET);

    let messages = instances.frame_input(PASS, &FrameOverlay::default());

    assert!(matches!(
        messages.as_slice(),
        [Message::Input(batch)] if batch.events == [InputEvent::Focus(true)]
    ));

    instances.screen_set(Vec::new());
    host::clear_focus();

    assert!(
        instances
            .frame_input(PASS, &FrameOverlay::default())
            .is_empty()
    );
}
