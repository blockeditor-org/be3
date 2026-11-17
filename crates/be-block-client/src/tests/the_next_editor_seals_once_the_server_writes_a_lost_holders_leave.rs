use be_block_session::Membership;
use be_type_text::{TextBlock, TextContent};

use super::*;
use crate::Event;

/// Only the server writes leaves: a holder whose connection is lost stays the holder until the
/// server sees the socket close and writes its leave, and from that entry the next editor holds the lease and seals.
#[tokio::test]
async fn the_next_editor_seals_once_the_server_writes_a_lost_holders_leave() {
    let harness = Harness::start().await;
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

    relay.cut();
    while seen.has(holder) {
        if let Event::Membership { block, entry } = on_laptop.recv().await.unwrap()
            && block == note
        {
            seen.apply(&entry);
        }
    }
    assert!(seen.is_holder(next), "the next editor holds the lease");

    let sealed = TextBlock::of("sealed by the next editor");
    let Saved::Published(_) = laptop
        .write(note, sealing(&sealed, head, 1))
        .await
        .unwrap()
    else {
        panic!("the next editor's seal is taken");
    };
    assert_eq!(
        server_journal(&laptop, note).await,
        None,
        "the next editor's seal trimmed the journal"
    );
    harness.stop().await;
}
