use std::sync::Mutex;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};

use be_sdk_api::{HostTrayItem, TrayClick, TrayRequest};
use futures_util::StreamExt;
use tokio::sync::mpsc::unbounded_channel;
use zbus::{Connection, MatchRule, MessageStream};

use super::super::client::{Event, serve};
use crate::dbus::test_bus::{PrivateBus, next, within};

struct Indicator;

#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl Indicator {
    #[zbus(property)]
    fn id(&self) -> String {
        "indicator".to_owned()
    }

    #[zbus(property)]
    fn status(&self) -> String {
        "Active".to_owned()
    }

    fn secondary_activate(&self, _x: i32, _y: i32) {}
}

struct Bare {
    heard: Mutex<SyncSender<String>>,
}

#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl Bare {
    #[zbus(property)]
    fn id(&self) -> String {
        "bare".to_owned()
    }

    #[zbus(property)]
    fn status(&self) -> String {
        "Active".to_owned()
    }

    fn activate(&self, _x: i32, _y: i32) -> zbus::fdo::Result<()> {
        Err(zbus::fdo::Error::UnknownMethod(
            "no Activate here".to_owned(),
        ))
    }

    fn context_menu(&self, x: i32, y: i32) {
        let _ = self
            .heard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .send(format!("ContextMenu {x} {y}"));
    }
}

async fn register(item: &Connection) -> String {
    item.call_method(
        Some("org.kde.StatusNotifierWatcher"),
        "/StatusNotifierWatcher",
        Some("org.kde.StatusNotifierWatcher"),
        "RegisterStatusNotifierItem",
        &("/StatusNotifierItem",),
    )
    .await
    .expect("the watcher takes the item");
    format!(
        "{}/StatusNotifierItem",
        item.unique_name().expect("the item has a name")
    )
}

fn until(
    published: &Receiver<Vec<HostTrayItem>>,
    what: &str,
    done: impl Fn(&[HostTrayItem]) -> bool,
) -> Vec<HostTrayItem> {
    loop {
        let items = next(published, &format!("the tray shows {what}"));
        if done(&items) {
            return items;
        }
    }
}

fn is_menu(items: &[HostTrayItem], key: &str) -> Option<bool> {
    items
        .iter()
        .find(|item| item.id == key)
        .map(|item| item.item_is_menu)
}

#[test]
fn an_item_without_activate_is_only_a_menu() {
    let bus = PrivateBus::start();
    let (published_sender, published) = std::sync::mpsc::channel();
    let (heard_sender, heard) = sync_channel(16);
    let (events, events_heard) = unbounded_channel();
    zbus::block_on(async {
        let indicator = bus.connect().await;
        let bare = bus.connect().await;
        let watcher_rule = MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .member("NameOwnerChanged")
            .expect("a member")
            .arg(0, "org.kde.StatusNotifierWatcher")
            .expect("an argument")
            .build();
        let mut watcher_up = MessageStream::for_match_rule(watcher_rule, &indicator, None)
            .await
            .expect("the item can watch for the watcher");
        let shared = bus.shared();
        let sender = events.clone();
        crate::dbus::spawn(async move {
            serve(&shared, sender, events_heard, move |items| {
                let _ = published_sender.send(items);
            })
            .await;
        });
        within("the watcher comes up", watcher_up.next()).await;

        indicator
            .object_server()
            .at("/StatusNotifierItem", Indicator)
            .await
            .expect("the indicator is served");
        bare.object_server()
            .at(
                "/StatusNotifierItem",
                Bare {
                    heard: Mutex::new(heard_sender),
                },
            )
            .await
            .expect("the bare item is served");
        let indicator_key = register(&indicator).await;
        let bare_key = register(&bare).await;

        let items = until(&published, "both items", |items| items.len() == 2);
        assert_eq!(
            is_menu(&items, &indicator_key),
            Some(true),
            "an item whose introspection has no Activate is only a menu"
        );
        assert_eq!(
            is_menu(&items, &bare_key),
            Some(false),
            "an item that says it has Activate is clicked"
        );

        let click = TrayClick {
            item: bare_key.clone(),
            x: 1700,
            y: 1050,
        };
        let _ = events.send(Event::Request(TrayRequest::Activate(click.clone())));
        until(&published, "the bare item as only a menu", |items| {
            is_menu(items, &bare_key) == Some(true)
        });
        let _ = events.send(Event::Request(TrayRequest::MenuOpened(click)));
        assert_eq!(
            next(&heard, "the item hears ContextMenu"),
            "ContextMenu 1700 1050",
            "an item without a menu of its own is asked to show one where the click was"
        );
    });
}
