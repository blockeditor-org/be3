use super::*;

#[test]
fn tray_messages_round_trip() {
    let entries = vec![
        TrayMenuEntry {
            item: TrayMenuItem {
                id: 1,
                label: "Show".into(),
                enabled: true,
                ..TrayMenuItem::default()
            },
            children: Vec::new(),
        },
        TrayMenuEntry {
            item: TrayMenuItem {
                id: 2,
                separator: true,
                ..TrayMenuItem::default()
            },
            children: Vec::new(),
        },
        TrayMenuEntry {
            item: TrayMenuItem {
                id: 3,
                label: "Status".into(),
                enabled: true,
                ..TrayMenuItem::default()
            },
            children: vec![TrayMenuEntry {
                item: TrayMenuItem {
                    id: 4,
                    label: "Away".into(),
                    enabled: true,
                    toggle: TrayToggle::Radio(true),
                    ..TrayMenuItem::default()
                },
                children: Vec::new(),
            }],
        },
    ];
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
        menu: TrayMenuEntry::flatten(&entries),
    };
    assert_eq!(
        item.menu.iter().map(|item| item.depth).collect::<Vec<_>>(),
        vec![0, 0, 0, 1],
        "a submenu's entries follow it one deeper"
    );
    assert_eq!(item.entries(), Ok(entries), "the flat menu nests back");
    watched_round_trips::<TrayWatch>(vec![item]);
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
        round_trips(&request);
    }
}
