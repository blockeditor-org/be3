use super::*;
use block_plugin_api::{HostWindow, HostWindowId, LinuxMessage, Size};

fn windows_sent(messages: &[Message]) -> Vec<Vec<HostWindow>> {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::Editor(EditorMessage::Linux {
                instance,
                message: LinuxMessage::Windows(windows),
            }) if *instance == INSTANCE => Some(windows.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_windows_the_host_runs_reach_an_instance_again_after_a_restart() {
    let mut instances = placed();
    let windows = vec![HostWindow {
        id: HostWindowId(1),
        title: "Terminal".to_owned(),
        app_id: "foot".to_owned(),
        parent: None,
        size: Size {
            width: 640.0,
            height: 480.0,
        },
        fullscreen: None,
        responding: true,
    }];
    instances.next_screens(PASS);

    assert!(instances.set_windows(INSTANCE, windows.clone()));
    assert_eq!(
        windows_sent(&instances.next_screens(PASS).opened),
        vec![windows.clone()]
    );
    assert!(
        !instances.set_windows(INSTANCE, windows.clone()),
        "an unchanged list is not news"
    );
    assert!(windows_sent(&instances.next_screens(PASS).opened).is_empty());

    instances.reopen();

    assert_eq!(
        windows_sent(&instances.next_screens(PASS).opened),
        vec![windows],
        "a restarted plugin is told again"
    );
}
