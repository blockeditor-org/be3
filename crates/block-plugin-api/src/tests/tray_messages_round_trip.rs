use super::*;

#[test]
fn tray_messages_round_trip() {
    let item = HostTrayItem {
        id: ":1.42/StatusNotifierItem".into(),
        title: "Chat".into(),
        tooltip: "3 unread".into(),
        status: TrayStatus::NeedsAttention,
        icon: Some(HostImage {
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 4],
        }),
        item_is_menu: false,
        menu: vec![
            TrayMenuEntry {
                id: 1,
                label: "Show".into(),
                enabled: true,
                ..TrayMenuEntry::default()
            },
            TrayMenuEntry {
                id: 2,
                separator: true,
                ..TrayMenuEntry::default()
            },
            TrayMenuEntry {
                id: 3,
                label: "Status".into(),
                enabled: true,
                children: vec![TrayMenuEntry {
                    id: 4,
                    label: "Away".into(),
                    enabled: true,
                    toggle: TrayToggle::Radio(true),
                    ..TrayMenuEntry::default()
                }],
                ..TrayMenuEntry::default()
            },
        ],
    };
    host_value_round_trips::<Tray>(vec![item]);
    for request in [
        TrayRequest::Activate("a".into()),
        TrayRequest::SecondaryActivate("a".into()),
        TrayRequest::Scroll {
            item: "a".into(),
            delta: -120,
            horizontal: true,
        },
        TrayRequest::MenuOpened("a".into()),
        TrayRequest::MenuClosed("a".into()),
        TrayRequest::MenuChosen {
            item: "a".into(),
            entry: 4,
        },
    ] {
        host_action_round_trips(request);
    }
}
