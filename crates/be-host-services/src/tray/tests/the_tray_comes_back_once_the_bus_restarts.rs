use std::sync::mpsc::Receiver;

use be_sdk_api::HostTrayItem;
use tokio::sync::mpsc::unbounded_channel;
use zbus::Connection;

use super::super::client::serve;
use crate::dbus::test_bus::{PrivateBus, next, owned, within};

const WATCHER: &str = "org.kde.StatusNotifierWatcher";

struct Item {
    title: &'static str,
}

#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl Item {
    #[zbus(property)]
    fn id(&self) -> String {
        self.title.to_lowercase()
    }

    #[zbus(property)]
    fn title(&self) -> String {
        self.title.to_owned()
    }

    #[zbus(property)]
    fn status(&self) -> String {
        "Active".to_owned()
    }

    fn activate(&self, _x: i32, _y: i32) {}
}

async fn item(bus: &PrivateBus, title: &'static str) -> Connection {
    let item = bus.connect().await;
    owned(&item, WATCHER).await;
    item.object_server()
        .at("/StatusNotifierItem", Item { title })
        .await
        .expect("the item is served");
    within(
        "the watcher takes the item",
        item.call_method(
            Some(WATCHER),
            "/StatusNotifierWatcher",
            Some(WATCHER),
            "RegisterStatusNotifierItem",
            &("/StatusNotifierItem",),
        ),
    )
    .await
    .expect("the watcher takes the item");
    item
}

fn until(published: &Receiver<Vec<HostTrayItem>>, what: &str, done: impl Fn(&[HostTrayItem]) -> bool) {
    while !done(&next(published, &format!("the tray shows {what}"))) {}
}

fn titled(title: &str) -> impl Fn(&[HostTrayItem]) -> bool {
    move |items| items.len() == 1 && items[0].title == title
}

#[test]
fn the_tray_comes_back_once_the_bus_restarts() {
    let mut bus = PrivateBus::start();
    let (published_sender, published) = std::sync::mpsc::channel();
    let (events, events_heard) = unbounded_channel();
    let shared = bus.shared();
    crate::dbus::spawn(async move {
        serve(&shared, events, events_heard, move |items| {
            let _ = published_sender.send(items);
        })
        .await;
    });
    zbus::block_on(async {
        let _chat = item(&bus, "Chat").await;
        until(&published, "the item", titled("Chat"));
    });

    bus.kill();
    until(&published, "nothing once the bus is gone", <[HostTrayItem]>::is_empty);
    bus.start_again();

    zbus::block_on(async {
        let _mail = item(&bus, "Mail").await;
        until(&published, "the item on the new bus", titled("Mail"));
    });
}
