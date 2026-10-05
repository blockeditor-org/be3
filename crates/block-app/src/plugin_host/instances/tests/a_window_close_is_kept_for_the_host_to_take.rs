use super::*;
use block_plugin_api::HostWindowId;

#[test]
fn a_window_close_is_kept_for_the_host_to_take() {
    let mut instances = placed();
    instances.next_screens(PASS);

    assert!(instances.editor_message(EditorMessage::CloseWindow {
        instance: INSTANCE,
        window: HostWindowId(4),
    }));

    assert_eq!(
        instances.take_closed_windows(INSTANCE),
        vec![HostWindowId(4)]
    );
    assert!(instances.take_closed_windows(INSTANCE).is_empty());
}
