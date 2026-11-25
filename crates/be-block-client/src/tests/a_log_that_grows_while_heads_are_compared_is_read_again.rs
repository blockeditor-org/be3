use super::*;

/// The server's proof that a shorter head lies on the log is asked against the head this
/// device just read; when the log grew between the two, the proof names a longer head and the
/// comparison reads the log again rather than taking it for a fork.
#[tokio::test]
async fn a_log_that_grows_while_heads_are_compared_is_read_again() {
    let harness = Harness::start().await;
    let laptop = harness.owner("owner@example.com").await;
    let chat = workspace(&laptop, "Chat").await;
    let append = |text: &'static str| {
        let laptop = &laptop;
        async move {
            laptop
                .append(
                    chat,
                    "test.app",
                    text.as_bytes(),
                    be_block_content::Along::default(),
                )
                .await
                .unwrap();
        }
    };
    append("one").await;
    append("two").await;
    laptop.read_log(chat, 0, 10).await.unwrap();
    let older = laptop.signed_log_head(chat).unwrap();
    append("three").await;

    let server = harness
        .url
        .trim_start_matches("ws://")
        .parse()
        .expect("the server listens on an address");
    let relay = be_block_fault_relay::Relay::start(server);
    let tablet = std::sync::Arc::new(
        Peer::connect(
            PeerConfig::new(
                &relay.url(),
                laptop.keys().identity().clone(),
                Credentials::Token(laptop.token().to_owned()),
            )
            .organization(Some(laptop.organization())),
            MemoryStore::new(),
        )
        .await
        .unwrap(),
    );
    tablet.read_log(chat, 0, 10).await.unwrap();
    let mut reads = relay.count(|method, _| method == be_block_protocol::__args::SERVER_READ);
    // The tablet's first read of the head is answered before the laptop appends, and held
    // until after it.
    relay.hold_answers(true);
    let heard = relay.heard();
    let comparing = {
        let tablet = std::sync::Arc::clone(&tablet);
        let older = older.clone();
        tokio::spawn(async move { tablet.compare_log_head(chat, older, HeadVia::Session).await })
    };
    reads.wait_for(|reads| *reads >= 1).await.unwrap();
    // The server answered it, and its answer waits at the relay.
    relay.heard_beyond(heard).await;
    append("four").await;
    relay.hold_answers(false);
    assert_eq!(
        comparing.await.unwrap().unwrap(),
        LogAgreement::Agrees,
        "a log that grew meanwhile was taken for a fork"
    );
    assert!(
        *reads.borrow() >= 4,
        "the head was not read again once the proof named a longer one: {} reads",
        *reads.borrow()
    );
    assert_eq!(tablet.log_fork(chat), None);
    harness.stop().await;
}
