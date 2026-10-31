use std::sync::Mutex;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};

use block_plugin_api::{HostTrayItem, TrayRequest};
use futures_util::StreamExt;
use tokio::sync::mpsc::unbounded_channel;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::OwnedObjectPath;
use zbus::{MatchRule, MessageStream};

use super::super::client::{Event, run_on};
use super::*;
use crate::dbus::test_bus::Bus;

const MENU_PATH: &str = "/MenuBar";

struct FakeItem {
    title: String,
    heard: Mutex<SyncSender<String>>,
}

impl FakeItem {
    fn hear(&self, what: String) {
        let _ = self
            .heard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .send(what);
    }
}

#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl FakeItem {
    #[zbus(property)]
    fn id(&self) -> String {
        "chat".to_owned()
    }

    #[zbus(property)]
    fn title(&self) -> String {
        self.title.clone()
    }

    #[zbus(property)]
    fn status(&self) -> String {
        "Active".to_owned()
    }

    #[zbus(property)]
    fn icon_pixmap(&self) -> Vec<(i32, i32, Vec<u8>)> {
        vec![(2, 2, [0xff, 0x58, 0x65, 0xf2].repeat(4))]
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn menu(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from(MENU_PATH).expect("a path")
    }

    fn activate(&self, x: i32, y: i32) {
        self.hear(format!("Activate {x} {y}"));
    }

    fn scroll(&self, delta: i32, orientation: String) {
        self.hear(format!("Scroll {delta} {orientation}"));
    }

    #[zbus(signal)]
    async fn new_title(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

struct FakeMenu {
    labels: Vec<(i32, String)>,
    heard: Mutex<SyncSender<String>>,
}

type Layout = (u32, (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>));

#[zbus::interface(name = "com.canonical.dbusmenu")]
impl FakeMenu {
    fn get_layout(&self, _parent: i32, _depth: i32, _names: Vec<String>) -> Layout {
        let children = self
            .labels
            .iter()
            .map(|(id, label)| {
                owned(entry(
                    *id,
                    vec![("label", Value::from(label.clone()))],
                    vec![],
                ))
            })
            .collect();
        (1, (0, HashMap::new(), children))
    }

    fn about_to_show(&self, id: i32) -> bool {
        self.heard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .send(format!("AboutToShow {id}"))
            .ok();
        false
    }

    fn event(&self, id: i32, event: String, _data: OwnedValue, _timestamp: u32) {
        let _ = self
            .heard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .send(format!("Event {id} {event}"));
    }

    #[zbus(signal)]
    async fn layout_updated(
        emitter: &SignalEmitter<'_>,
        revision: u32,
        parent: i32,
    ) -> zbus::Result<()>;
}

fn until(
    published: &Receiver<Vec<HostTrayItem>>,
    what: &str,
    done: impl Fn(&[HostTrayItem]) -> bool,
) -> Vec<HostTrayItem> {
    loop {
        let items = published
            .recv()
            .unwrap_or_else(|_| panic!("the tray shows {what}"));
        if done(&items) {
            return items;
        }
    }
}

fn heard(heard: &Receiver<String>, what: &str) {
    let got = heard
        .recv()
        .unwrap_or_else(|_| panic!("the item hears {what}"));
    assert_eq!(got, what);
}

#[test]
fn items_on_a_private_bus_show_and_answer_the_desktop() {
    let bus = Bus::start();
    let (published_sender, published) = std::sync::mpsc::channel();
    let (heard_sender, heard_by_item) = sync_channel(16);
    let (events, events_heard) = unbounded_channel();
    zbus::block_on(async {
        let host = bus.connect().await;
        let item = bus.connect().await;
        let watcher_rule = MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .member("NameOwnerChanged")
            .expect("a member")
            .arg(0, "org.kde.StatusNotifierWatcher")
            .expect("an argument")
            .build();
        let mut watcher_up = MessageStream::for_match_rule(watcher_rule, &item, None)
            .await
            .expect("the item can watch for the watcher");
        crate::dbus::spawn(run_on(host, events.clone(), events_heard, move |items| {
            let _ = published_sender.send(items);
        }));
        watcher_up.next().await;

        item.object_server()
            .at(
                "/StatusNotifierItem",
                FakeItem {
                    title: "Chat".to_owned(),
                    heard: Mutex::new(heard_sender.clone()),
                },
            )
            .await
            .expect("the item is served");
        item.object_server()
            .at(
                MENU_PATH,
                FakeMenu {
                    labels: vec![(1, "_Open".to_owned()), (2, "Quit".to_owned())],
                    heard: Mutex::new(heard_sender),
                },
            )
            .await
            .expect("the menu is served");
        item.call_method(
            Some("org.kde.StatusNotifierWatcher"),
            "/StatusNotifierWatcher",
            Some("org.kde.StatusNotifierWatcher"),
            "RegisterStatusNotifierItem",
            &("/StatusNotifierItem",),
        )
        .await
        .expect("the watcher takes the item");
        let key = format!(
            "{}/StatusNotifierItem",
            item.unique_name().expect("the item has a name")
        );

        let items = until(&published, "the item with its menu", |items| {
            items.first().is_some_and(|item| item.menu.len() == 2)
        });
        assert_eq!(items[0].id, key);
        assert_eq!(items[0].title, "Chat");
        assert_eq!(items[0].menu[0].label, "Open");
        let icon = items[0].icon.as_ref().expect("the pixmap is the icon");
        assert_eq!((icon.width, icon.height), (2, 2));
        assert_eq!(&icon.rgba[..4], &[0x58, 0x65, 0xf2, 0xff]);

        let _ = events.send(Event::Request(TrayRequest::Activate(key.clone())));
        heard(&heard_by_item, "Activate 0 0");
        let _ = events.send(Event::Request(TrayRequest::Scroll {
            item: key.clone(),
            delta: -120,
            horizontal: false,
        }));
        heard(&heard_by_item, "Scroll -120 vertical");
        let _ = events.send(Event::Request(TrayRequest::MenuOpened(key.clone())));
        heard(&heard_by_item, "AboutToShow 0");
        heard(&heard_by_item, "Event 0 opened");
        let _ = events.send(Event::Request(TrayRequest::MenuChosen {
            item: key.clone(),
            entry: 2,
        }));
        heard(&heard_by_item, "Event 2 clicked");

        let served = item
            .object_server()
            .interface::<_, FakeItem>("/StatusNotifierItem")
            .await
            .expect("the item is served");
        served.get_mut().await.title = "Chat (3)".to_owned();
        FakeItem::new_title(served.signal_emitter())
            .await
            .expect("the item signals");
        until(&published, "the new title", |items| {
            items.first().is_some_and(|item| item.title == "Chat (3)")
        });

        let menu = item
            .object_server()
            .interface::<_, FakeMenu>(MENU_PATH)
            .await
            .expect("the menu is served");
        menu.get_mut().await.labels.push((3, "Settings".to_owned()));
        FakeMenu::layout_updated(menu.signal_emitter(), 2, 0)
            .await
            .expect("the menu signals");
        until(&published, "the changed menu", |items| {
            items.first().is_some_and(|item| item.menu.len() == 3)
        });

        drop(served);
        drop(menu);
        item.close().await.expect("the program leaves the bus");
        until(
            &published,
            "nothing once the program goes",
            <[HostTrayItem]>::is_empty,
        );
    });
}
