use super::*;

use block_plugin_api::{
    HostWindowId, MediaRequest, NotificationReport, NotificationSignal, PlayerCommand, PowerAction,
    WindowAction,
};

#[test]
fn host_actions_reach_the_host_only_from_the_shell() {
    let mut instances = placed();
    instances.next_screens(PASS);

    assert!(
        !act(&mut instances, &PowerAction::PowerOff),
        "an editor that is not the shell may not ask"
    );
    assert!(take_actions::<PowerAction>(&mut instances).is_empty());

    instances.set_shell(Some(INSTANCE));
    let media = [
        MediaRequest::StepVolume(0.05),
        MediaRequest::SetVolume(0.3),
        MediaRequest::Player(PlayerCommand::Next),
    ];
    for request in &media {
        assert!(act(&mut instances, request));
    }
    assert!(act(&mut instances, &PowerAction::Suspend));
    let notification = NotificationReport {
        received: Some(3),
        signals: vec![NotificationSignal::ActionInvoked(2, "default".to_owned())],
        kept: vec![2],
    };
    assert!(act(&mut instances, &notification));
    assert!(act(&mut instances, &WindowAction::Close(HostWindowId(4))));

    assert_eq!(
        take_actions::<MediaRequest>(&mut instances),
        media.to_vec(),
        "each kind of action is taken by its own key, in the order it was asked"
    );
    assert!(take_actions::<MediaRequest>(&mut instances).is_empty());
    assert_eq!(
        take_actions::<PowerAction>(&mut instances),
        vec![PowerAction::Suspend]
    );
    assert_eq!(
        take_actions::<NotificationReport>(&mut instances),
        vec![notification]
    );
    assert_eq!(
        take_actions::<WindowAction>(&mut instances),
        vec![WindowAction::Close(HostWindowId(4))]
    );

    while act(&mut instances, &MediaRequest::ToggleMute) {}
    assert!(
        act(&mut instances, &WindowAction::Focus(HostWindowId(4))),
        "a full queue of one kind of action leaves room for the others"
    );
    assert_eq!(
        take_actions::<WindowAction>(&mut instances),
        vec![WindowAction::Focus(HostWindowId(4))]
    );
    assert_eq!(
        take_actions::<MediaRequest>(&mut instances).len(),
        crate::plugin_host::host_values::MAX_ACTIONS,
        "actions nobody takes are held only up to a limit"
    );
    assert!(
        !instances.editor_message(EditorMessage::HostAction {
            instance: INSTANCE,
            key: "nothing the host does".to_owned(),
            action: Vec::new(),
        }),
        "an action the host does not know is refused"
    );
}
