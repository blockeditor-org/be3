use super::*;

use block_plugin_api::{
    HostWindowId, MediaRequest, MediaWatch, NotificationReport, NotificationSignal,
    NotificationsWatch, PlayerCommand, PowerAction, PowerWatch, WindowAction, WindowsWatch,
    requested,
};

#[test]
fn host_calls_are_taken_by_kind_in_the_order_they_came() {
    let mut instances = placed();
    instances.next_screens(PASS);

    let media = [
        MediaRequest::StepVolume(0.05),
        MediaRequest::SetVolume(0.3),
        MediaRequest::Player(PlayerCommand::Next),
    ];
    for request in &media {
        assert!(host_call::<MediaWatch, _>(
            &mut instances,
            "request",
            request
        ));
    }
    assert!(host_call::<PowerWatch, _>(
        &mut instances,
        "request",
        &PowerAction::Suspend
    ));
    let notification = NotificationReport {
        received: Some(3),
        signals: vec![NotificationSignal::ActionInvoked {
            id: 2,
            key: "default".to_owned(),
        }],
        kept: vec![2],
    };
    assert!(host_call::<NotificationsWatch, _>(
        &mut instances,
        "report",
        &notification
    ));
    let close = WindowAction::Close(HostWindowId(4));
    assert!(host_call::<WindowsWatch, _>(
        &mut instances,
        "request",
        &close
    ));

    assert_eq!(
        take(&mut instances, requested::media),
        media.to_vec(),
        "each kind of call is taken by its own reader, in the order it was made"
    );
    assert!(take(&mut instances, requested::media).is_empty());
    assert_eq!(
        take(&mut instances, requested::power),
        vec![PowerAction::Suspend]
    );
    assert_eq!(
        take(&mut instances, requested::notification_report),
        vec![notification]
    );
    assert_eq!(take(&mut instances, requested::window), vec![close]);

    while host_call::<MediaWatch, _>(&mut instances, "request", &MediaRequest::ToggleMute) {}
    let focus = WindowAction::Focus(HostWindowId(4));
    assert!(
        host_call::<WindowsWatch, _>(&mut instances, "request", &focus),
        "a full queue of one kind of call leaves room for the others"
    );
    assert_eq!(take(&mut instances, requested::window), vec![focus]);
    assert_eq!(
        take(&mut instances, requested::media).len(),
        crate::published::MAX_CALLS,
        "calls nobody takes are held only up to a limit"
    );
}

fn take<C>(
    instances: &mut Instances,
    accept: impl Fn(&block_plugin_api::ops::Started) -> Option<C>,
) -> Vec<C> {
    let mut taken = Vec::new();
    instances.take_calls(&mut |called| match accept(called) {
        Some(call) => {
            taken.push(call);
            true
        }
        None => false,
    });
    taken
}
