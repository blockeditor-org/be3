use block_editor_beui::beui::{PointerButton, Vec2};
use block_editor_beui::{
    HostTrayItem, TrayMenuItem, TrayRequest, TrayStatus, TrayToggle, TrayWatch,
};

use super::*;

const CHAT: &str = ":1.42/StatusNotifierItem";
const SYNC: &str = "org.example.Sync/StatusNotifierItem";
const HIDDEN: &str = ":1.50/StatusNotifierItem";

fn disc(rgb: [u8; 3]) -> HostImage {
    const SIDE: u32 = 32;
    let center = (SIDE as f32 - 1.0) / 2.0;
    let rgba = (0..SIDE * SIDE)
        .flat_map(|at| {
            let (x, y) = ((at % SIDE) as f32 - center, (at / SIDE) as f32 - center);
            let inside = x * x + y * y <= center * center;
            match inside {
                true => [rgb[0], rgb[1], rgb[2], 0xff],
                false => [0, 0, 0, 0],
            }
        })
        .collect();
    HostImage {
        width: SIDE,
        height: SIDE,
        rgba,
    }
}

fn entry(id: i32, label: &str) -> TrayMenuItem {
    TrayMenuItem {
        id,
        label: label.to_owned(),
        enabled: true,
        ..TrayMenuItem::default()
    }
}

fn tray() -> Vec<HostTrayItem> {
    vec![
        HostTrayItem {
            id: CHAT.to_owned(),
            title: "Chat".to_owned(),
            tooltip: "3 unread".to_owned(),
            status: TrayStatus::Active,
            icon: Some(disc([0x58, 0x65, 0xf2])),
            item_is_menu: false,
            menu: vec![
                entry(1, "Open Chat"),
                TrayMenuItem {
                    toggle: TrayToggle::Check(true),
                    ..entry(2, "Start with the computer")
                },
                entry(3, "Status"),
                TrayMenuItem {
                    depth: 1,
                    toggle: TrayToggle::Radio(true),
                    ..entry(4, "Online")
                },
                TrayMenuItem {
                    depth: 1,
                    toggle: TrayToggle::Radio(false),
                    ..entry(5, "Away")
                },
                TrayMenuItem {
                    separator: true,
                    ..entry(6, "")
                },
                TrayMenuItem {
                    enabled: false,
                    ..entry(7, "Updates")
                },
                entry(8, "Quit"),
            ],
        },
        HostTrayItem {
            id: SYNC.to_owned(),
            title: "Sync".to_owned(),
            icon: Some(disc([0x2e, 0x7d, 0x32])),
            item_is_menu: true,
            ..HostTrayItem::default()
        },
        HostTrayItem {
            id: HIDDEN.to_owned(),
            title: "Hidden".to_owned(),
            status: TrayStatus::Passive,
            ..HostTrayItem::default()
        },
    ]
}

fn item(id: &str) -> String {
    format!("desktop.tray.{id}")
}

#[test]
fn the_tray_shows_its_items_and_answers_their_clicks_and_menus() {
    let mut fixture = Fixture::new();
    fixture.settle();
    assert!(
        fixture.test.watches::<TrayWatch>(),
        "the desktop asks the host for the tray"
    );
    fixture.test.set_watched::<TrayWatch>(&tray());
    fixture.settle();
    assert!(fixture.test.shown(&item(CHAT)));
    assert!(fixture.test.shown(&item(SYNC)));
    assert!(
        !fixture.test.shown(&item(HIDDEN)),
        "a passive item stays out of the bar"
    );
    fixture.test.snapshot("the_tray_in_the_bar");

    fixture.test.click(item(CHAT));
    fixture.settle();
    fixture.test.click_with(item(CHAT), PointerButton::Middle);
    fixture.settle();
    fixture.test.scroll(item(CHAT), Vec2::new(0.0, -40.0));
    fixture.settle();
    assert_eq!(
        fixture.test.take_host_calls(block_ui_test::requested::tray),
        vec![
            TrayRequest::Activate(CHAT.to_owned()),
            TrayRequest::SecondaryActivate(CHAT.to_owned()),
            TrayRequest::Scroll {
                item: CHAT.to_owned(),
                delta: 120,
                horizontal: false,
            },
        ],
        "a click activates, a middle click activates the other way and the wheel scrolls"
    );
    assert!(!fixture.test.shown(&format!("{}.1", item(CHAT))));

    fixture
        .test
        .click_with(item(CHAT), PointerButton::Secondary);
    fixture.settle();
    fixture.test.settle();
    assert!(
        fixture.test.shown(&format!("{}.1", item(CHAT))),
        "a right click opens the menu"
    );
    fixture.test.hover(format!("{}.3", item(CHAT)));
    fixture.settle();
    fixture.test.settle();
    assert!(
        fixture.test.shown(&format!("{}.5", item(CHAT))),
        "a submenu opens on hover"
    );
    fixture.test.snapshot("the_tray_menu");
    fixture.test.click(format!("{}.5", item(CHAT)));
    fixture.settle();
    fixture.test.settle();
    assert_eq!(
        fixture.test.take_host_calls(block_ui_test::requested::tray),
        vec![
            TrayRequest::MenuOpened(CHAT.to_owned()),
            TrayRequest::MenuChosen {
                item: CHAT.to_owned(),
                entry: 5,
            },
            TrayRequest::MenuClosed(CHAT.to_owned()),
        ],
        "the host hears the menu open, the choice and the menu close"
    );
    assert!(!fixture.test.shown(&format!("{}.1", item(CHAT))));

    fixture.test.click(item(SYNC));
    fixture.settle();
    fixture
        .test
        .click_with(item(SYNC), PointerButton::Secondary);
    fixture.settle();
    assert_eq!(
        fixture.test.take_host_calls(block_ui_test::requested::tray),
        vec![
            TrayRequest::MenuOpened(SYNC.to_owned()),
            TrayRequest::MenuOpened(SYNC.to_owned()),
        ],
        "an item that is only a menu, with none to show, is asked to show its own"
    );

    let mut changed = tray();
    changed[1].menu = vec![entry(1, "Pause syncing")];
    changed.remove(0);
    fixture.test.set_watched::<TrayWatch>(&changed);
    fixture.settle();
    assert!(
        !fixture.test.shown(&item(CHAT)),
        "an item goes when its program does"
    );
    fixture.test.click(item(SYNC));
    fixture.settle();
    fixture.test.settle();
    assert!(
        fixture.test.shown(&format!("{}.1", item(SYNC))),
        "a click opens the menu of an item that is only a menu"
    );
}
