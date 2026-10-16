use super::*;

#[test]
fn a_device_waiting_on_remaps_asks_the_host_to_show_them() {
    let mut editor = remaps_editor(RemapState::Waiting);
    assert!(editor.shown("input-settings.remaps.review"));

    editor.click("input-settings.remaps.review");
    editor.run();

    assert_eq!(
        editor.take_actions::<RemapRequest>(),
        vec![RemapRequest::Review]
    );
    editor.snapshot("a_device_waiting_on_remaps_asks_the_host_to_show_them");

    editor.set_host_value::<Remaps>(&RemapState::On);
    editor.run();
    assert!(!editor.shown("input-settings.remaps.review"));
}
