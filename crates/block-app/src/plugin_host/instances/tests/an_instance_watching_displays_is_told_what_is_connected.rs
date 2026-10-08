use super::*;

use block_plugin_api::{HostDisplay, HostDisplayMode, LinuxMessage};

fn displays_sent(messages: &[Message]) -> Vec<Vec<HostDisplay>> {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::Editor(EditorMessage::Linux {
                instance,
                message: LinuxMessage::Displays(displays),
            }) if *instance == INSTANCE => Some(displays.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn an_instance_watching_displays_is_told_what_is_connected() {
    let fast = HostDisplayMode {
        width: 2560,
        height: 1440,
        refresh_millihertz: 239_970,
    };
    let monitor = HostDisplay {
        id: "DEL|DELL AW2524H|7XQ2B34".to_owned(),
        name: "DELL AW2524H".to_owned(),
        connector: "DP-1".to_owned(),
        modes: vec![fast],
        default: fast,
        current: fast,
    };
    let mut instances = placed();
    instances.next_screens(PASS);

    assert!(
        !instances.set_displays(vec![monitor.clone()]),
        "nobody is watching yet"
    );
    assert!(displays_sent(&instances.next_screens(PASS).opened).is_empty());

    assert!(instances.editor_message(EditorMessage::Linux {
        instance: INSTANCE,
        message: LinuxMessage::WatchDisplays,
    }));
    assert_eq!(
        displays_sent(&instances.next_screens(PASS).opened),
        vec![vec![monitor.clone()]]
    );
    assert!(displays_sent(&instances.next_screens(PASS).opened).is_empty());

    instances.reopen();
    assert_eq!(
        displays_sent(&instances.next_screens(PASS).opened),
        vec![vec![monitor]],
        "a restarted plugin is told again"
    );
}
