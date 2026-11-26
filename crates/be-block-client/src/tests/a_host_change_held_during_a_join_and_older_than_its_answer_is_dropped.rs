use be_type_example_counter::CounterContent;

use super::*;
use crate::Event;

/// A device claims a session's lease and rejoins, as one that drops its claim does: the
/// server sends the claim's change of host before it takes the join, which reaches the device
/// while the join is on its way. The join's answer, that no one hosts, stands: the held change
/// is older than it, so it is dropped rather than told after the answer, naming this device
/// host again.
#[tokio::test]
async fn a_host_change_held_during_a_join_and_older_than_its_answer_is_dropped() {
    let harness = Harness::start().await;
    let server = harness
        .url
        .trim_start_matches("ws://")
        .parse()
        .expect("the server listens on an address");
    let relay = be_block_fault_relay::Relay::start(server);
    let email = "laptop@example.com";
    let laptop = Peer::connect(
        PeerConfig::new(
            &relay.url(),
            identity(email),
            Credentials::Token(harness.server.register(email, email).await),
        )
        .chunker(ChunkerConfig::SMALL),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    let block = laptop
        .create::<CounterContent>(BlockParent::Root)
        .await
        .unwrap();
    let phone = harness.second(&laptop).await;
    let mut on_phone = phone.connection().subscribe();
    phone
        .connection()
        .join_session(phone.organization(), block, BUILD)
        .await
        .unwrap();
    let connection = laptop.connection();
    let (client, ..) = connection
        .join_session(laptop.organization(), block, BUILD)
        .await
        .unwrap();

    // What the server tells the laptop waits at the relay, so the claim's change of host
    // reaches it only once the rejoin is on its way.
    relay.hold_answers(true);
    let mut on_laptop = connection.subscribe();
    let claimed = connection.claim(block);
    loop {
        if let Event::Host {
            block: of,
            host: Some(host),
            ..
        } = on_phone.recv().await.unwrap()
            && of == block
            && host.client == client
        {
            break;
        }
    }
    let rejoined = connection.join_session(laptop.organization(), block, BUILD);
    relay.hold_answers(false);
    claimed.await.unwrap();
    let (again, ..) = rejoined.await.unwrap();
    assert_eq!(again, client, "a connection keeps its id in the session");

    // Something the laptop hears after all of that, to stop at.
    phone
        .connection()
        .relay(block, Some(client), b"after".to_vec())
        .await
        .unwrap();
    let mut told = Vec::new();
    loop {
        match on_laptop.recv().await.unwrap() {
            Event::Host {
                block: of,
                host,
                ours,
                joined,
            } if of == block => told.push((host.map(|host| host.client), ours, joined)),
            Event::Relayed { block: of, .. } if of == block => break,
            _ => {}
        }
    }
    assert_eq!(
        told,
        [(None, false, true)],
        "the rejoin's answer that no one hosts is the last word"
    );
}
