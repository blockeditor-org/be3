use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

use be_sdk_api::{NotificationRequest, NotificationSignal};
use futures_util::StreamExt;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use zbus::zvariant::OwnedValue;
use zbus::{Connection, MatchRule, MessageStream};

use super::super::inbox::Ids;
use super::super::server::{Requests, serve};
use crate::dbus::test_bus::{PrivateBus, next, owned, within};

const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

async fn notify(client: &Connection, summary: &str) -> u32 {
    let hints: HashMap<String, OwnedValue> = HashMap::new();
    within(
        "the server answers Notify",
        client.call_method(
            Some(NAME),
            PATH,
            Some(NAME),
            "Notify",
            &(
                "Chat",
                0_u32,
                "",
                summary,
                "",
                Vec::<String>::new(),
                hints,
                -1_i32,
            ),
        ),
    )
    .await
    .expect("the server takes the notification")
    .body()
    .deserialize::<u32>()
    .expect("an id")
}

fn shown(requests: &Receiver<NotificationRequest>, summary: &str) -> u32 {
    match next(requests, &format!("the desktop hears {summary}")) {
        NotificationRequest::Notify(incoming) => {
            assert_eq!(incoming.summary, summary);
            incoming.id
        }
        other => panic!("the desktop heard {other:?} instead of {summary}"),
    }
}

async fn invoked(client: &Connection, signals: &UnboundedSender<NotificationSignal>, id: u32) {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(NAME)
        .expect("an interface")
        .member("ActionInvoked")
        .expect("a member")
        .build();
    let mut heard = MessageStream::for_match_rule(rule, client, None)
        .await
        .expect("the client watches the server's signals");
    let _ = signals.send(NotificationSignal::ActionInvoked {
        id,
        key: "reply".to_owned(),
    });
    let message = within("the client hears ActionInvoked", heard.next())
        .await
        .expect("a signal")
        .expect("a readable signal");
    let (heard_id, key) = message
        .body()
        .deserialize::<(u32, String)>()
        .expect("an id and a key");
    assert_eq!((heard_id, key.as_str()), (id, "reply"));
}

#[test]
fn notifications_are_served_again_once_the_bus_restarts() {
    let mut bus = PrivateBus::start();
    let (request_sender, requests) = channel();
    let request_sender = Mutex::new(request_sender);
    let requests_heard: Requests = Arc::new(move |request| {
        let _ = request_sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .send(request);
    });
    let (signals, signals_heard) = unbounded_channel();
    let shared = bus.shared();
    crate::dbus::spawn(async move {
        let ids = Arc::new(Mutex::new(Ids::default()));
        serve(&shared, requests_heard, signals_heard, ids).await;
    });
    zbus::block_on(async {
        let client = bus.connect().await;
        owned(&client, NAME).await;
        let first = notify(&client, "Before").await;
        assert_eq!(shown(&requests, "Before"), first);
        invoked(&client, &signals, first).await;
    });

    bus.kill();
    bus.start_again();

    zbus::block_on(async {
        let client = bus.connect().await;
        owned(&client, NAME).await;
        let second = notify(&client, "After").await;
        assert_eq!(shown(&requests, "After"), second);
        assert_ne!(
            second, 1,
            "ids go on from the ones the desktop already shows"
        );
        invoked(&client, &signals, second).await;
    });
}
