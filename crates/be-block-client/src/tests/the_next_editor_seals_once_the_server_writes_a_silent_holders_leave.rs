use be_block_session::Membership;
use be_type_text::{TextBlock, TextContent};

use super::*;
use crate::Event;

/// Only the server writes leaves: a holder whose socket goes silent stays the holder until the
/// server gives up on the connection (`ServerConfig::idle_timeout`) and writes its leave, and
/// from that entry the next editor holds the lease and seals.
#[tokio::test]
async fn the_next_editor_seals_once_the_server_writes_a_silent_holders_leave() {
    let harness = Harness::configured(be_server::ServerConfig {
        idle_timeout: std::time::Duration::from_secs(2),
        ..be_server::ServerConfig::OPEN
    })
    .await;
    let server = harness
        .url
        .trim_start_matches("ws://")
        .parse()
        .expect("the server listens on an address");
    let relay = be_block_fault_relay::Relay::start(server);
    let email = "phone@example.com";
    let phone = Peer::connect(
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
    let note = phone.create::<TextContent>(BlockParent::Root).await.unwrap();
    let head = phone
        .save(note, &TextBlock::of("first"), None)
        .await
        .unwrap()
        .published()
        .unwrap();
    let laptop = harness.second(&phone).await;
    let mut on_laptop = laptop.connection().subscribe();
    let (holder, ..) = phone.connection().join_session(note, BUILD).await.unwrap();
    let (next, mut seen, ..) = laptop.connection().join_session(note, BUILD).await.unwrap();
    submitted(&phone, note, 1).await;
    assert!(seen.is_holder(holder));

    relay.stall();
    // The laptop keeps talking, so the server gives up only on the phone.
    loop {
        let heard = tokio::time::timeout(std::time::Duration::from_millis(250), on_laptop.recv());
        match heard.await {
            Ok(Ok(Event::Membership { block, entry })) if block == note => {
                seen.apply(&entry);
                if !seen.has(holder) {
                    break;
                }
            }
            Ok(Ok(_)) => {}
            Ok(Err(error)) => panic!("the laptop stopped hearing its connection: {error}"),
            Err(_) => {
                be_block_protocol::client::connection::ping(&laptop.connection())
                    .await
                    .unwrap();
            }
        }
    }
    assert!(seen.is_holder(next), "the next editor holds the lease");

    let sealed = TextBlock::of("sealed by the next editor");
    let Saved::Published(sealed) = laptop
        .write(note, sealing(&sealed, head, 1))
        .await
        .unwrap()
    else {
        panic!("the next editor's seal is taken");
    };
    assert_eq!(server_journal(&laptop, note).await, (Some(sealed), 1, vec![]));
    harness.stop().await;
}
