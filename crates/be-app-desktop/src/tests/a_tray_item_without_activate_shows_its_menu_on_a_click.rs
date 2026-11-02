use be_sdk_beui::{HostTrayItem, TrayMenuItem, TrayRequest, TrayStatus, TrayWatch};

use super::*;

const CLOUD: &str = ":1.60/org/ayatana/NotificationItem/cloud";
const BARE: &str = ":1.61/StatusNotifierItem";

fn tray(cloud_is_menu: bool, bare_is_menu: bool) -> Vec<HostTrayItem> {
    let entry = |id: i32, label: &str| TrayMenuItem {
        id,
        label: label.to_owned(),
        enabled: true,
        ..TrayMenuItem::default()
    };
    vec![
        HostTrayItem {
            id: CLOUD.to_owned(),
            title: "Cloud".to_owned(),
            status: TrayStatus::Active,
            item_is_menu: cloud_is_menu,
            menu: vec![entry(1, "Open folder"), entry(2, "Pause syncing")],
            ..HostTrayItem::default()
        },
        HostTrayItem {
            id: BARE.to_owned(),
            title: "Bare".to_owned(),
            status: TrayStatus::Active,
            item_is_menu: bare_is_menu,
            ..HostTrayItem::default()
        },
    ]
}

#[test]
fn a_tray_item_without_activate_shows_its_menu_on_a_click() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.set_watched::<TrayWatch>(&tray(false, false));
    fixture.settle();
    let (cloud, bare) = (tray_click(&fixture, CLOUD), tray_click(&fixture, BARE));
    let first_entry = format!("{}.1", tray_button(CLOUD));

    fixture.test.set_watched::<TrayWatch>(&tray(true, false));
    fixture.settle();
    fixture.test.settle();
    assert!(
        !fixture.test.shown(&first_entry),
        "an item that turns out to be only a menu without being clicked opens nothing"
    );
    fixture.test.set_watched::<TrayWatch>(&tray(false, false));
    fixture.settle();

    fixture.test.click(tray_button(CLOUD));
    fixture.settle();
    assert_eq!(
        fixture.test.take_host_calls(be_sdk_test::requested::tray),
        vec![TrayRequest::Activate(cloud.clone())]
    );
    assert!(!fixture.test.shown(&first_entry));
    fixture.test.set_watched::<TrayWatch>(&tray(true, false));
    fixture.settle();
    fixture.test.settle();
    assert!(
        fixture.test.shown(&first_entry),
        "when the host finds the item has no Activate, the click shows its menu"
    );
    fixture
        .test
        .snapshot("a_tray_item_without_activate_shows_its_menu_on_a_click");
    assert_eq!(
        fixture.test.take_host_calls(be_sdk_test::requested::tray),
        vec![TrayRequest::MenuOpened(cloud.clone())]
    );
    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();
    fixture.test.settle();
    assert!(!fixture.test.shown(&first_entry));
    fixture.test.take_host_calls(be_sdk_test::requested::tray);

    fixture.test.click(tray_button(CLOUD));
    fixture.settle();
    fixture.test.settle();
    assert!(
        fixture.test.shown(&first_entry),
        "from then on a click opens the menu at once"
    );
    assert_eq!(
        fixture.test.take_host_calls(be_sdk_test::requested::tray),
        vec![TrayRequest::MenuOpened(cloud)],
        "and no longer asks the item to activate"
    );
    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();
    fixture.test.settle();
    fixture.test.take_host_calls(be_sdk_test::requested::tray);

    fixture.test.click(tray_button(BARE));
    fixture.settle();
    fixture.test.set_watched::<TrayWatch>(&tray(true, true));
    fixture.settle();
    assert_eq!(
        fixture.test.take_host_calls(be_sdk_test::requested::tray),
        vec![
            TrayRequest::Activate(bare.clone()),
            TrayRequest::MenuOpened(bare),
        ],
        "an item with no menu to show is asked to show its own, where the click was"
    );
}
