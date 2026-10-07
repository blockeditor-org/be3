use super::*;

use block_plugin_api::HostInputDevice;

fn devices_sent(messages: &[Message]) -> Vec<Vec<HostInputDevice>> {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::Editor(EditorMessage::InputDevices { instance, devices })
                if *instance == INSTANCE =>
            {
                Some(devices.clone())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn an_instance_watching_input_devices_is_told_what_is_connected() {
    let mouse = HostInputDevice {
        name: "Mouse".to_owned(),
        vendor: 0x046d,
        product: 0xc52b,
        speed: Some(0.0),
        tap_to_click: None,
        natural_scroll: Some(false),
    };
    let mut instances = placed();
    instances.next_screens(PASS);

    assert!(
        !instances.set_input_devices(vec![mouse.clone()]),
        "nobody is watching yet"
    );
    assert!(devices_sent(&instances.next_screens(PASS).opened).is_empty());

    assert!(instances.editor_message(EditorMessage::WatchInputDevices { instance: INSTANCE }));
    assert_eq!(
        devices_sent(&instances.next_screens(PASS).opened),
        vec![vec![mouse.clone()]]
    );
    assert!(devices_sent(&instances.next_screens(PASS).opened).is_empty());

    assert!(instances.set_input_devices(Vec::new()));
    assert_eq!(
        devices_sent(&instances.next_screens(PASS).opened),
        vec![Vec::new()]
    );

    instances.reopen();
    assert_eq!(
        devices_sent(&instances.next_screens(PASS).opened),
        vec![Vec::new()],
        "a restarted plugin is told again"
    );
}
