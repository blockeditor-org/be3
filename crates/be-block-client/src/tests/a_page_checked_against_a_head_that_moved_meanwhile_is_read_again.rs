use super::*;

/// The head a device keeps only moves on to one proved to extend it as it is when kept: a page
/// that was on its way while a push moved the head is checked again against the head kept now,
/// so a server that answers the page from a fork of the pushed head is caught, and the pushed
/// head stays.
#[tokio::test]
async fn a_page_checked_against_a_head_that_moved_meanwhile_is_read_again() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let chat = workspace(&owner, "Chat").await;
    owner
        .append(chat, "test.app", b"one", be_block_content::Along::default())
        .await
        .unwrap();
    owner
        .append(chat, "test.app", b"two", be_block_content::Along::default())
        .await
        .unwrap();
    let reader = harness.second(&owner).await;
    let mut events = reader.connection().subscribe();
    let first = reader.read_log(chat, 0, 10).await.unwrap();
    assert_eq!(first.head.size, 2);

    owner
        .append(
            chat,
            "test.app",
            b"three",
            be_block_content::Along::default(),
        )
        .await
        .unwrap();
    let pushed = loop {
        if let Ok(crate::Event::Appended(appended)) = events.recv().await {
            break appended;
        }
    };
    assert_eq!((pushed.previous, pushed.head.size), (2, 3));

    // The server forks after the first two entries: the same two, then another third.
    let database = harness.database();
    be_server::log::forget(&database, owner.organization(), chat).unwrap();
    for logged in &first.entries {
        let entry = &logged.entry;
        be_server::log::append(
            &database,
            owner.organization(),
            chat,
            entry.author,
            vec![be_block_protocol::NewEntry {
                id: entry.id,
                payload: entry.payload.clone().unwrap(),
                signature: entry.signature.clone(),
            }],
            entry.arrival,
        )
        .unwrap();
    }
    be_server::log::append(
        &database,
        owner.organization(),
        chat,
        owner.account(),
        vec![be_block_protocol::NewEntry {
            id: Uuid::new_v4(),
            payload: b"another third".to_vec(),
            signature: vec![1; 64],
        }],
        be_block_commit::now_milliseconds(),
    )
    .unwrap();
    drop(database);

    // The read asks while the head kept is the first two's; the push moves it before the
    // answer arrives.
    let (read, took) = tokio::join!(reader.read_log(chat, 0, 10), reader.logged(pushed.clone()));
    assert_eq!(took.unwrap().len(), 1);
    assert!(
        matches!(read, Err(ClientError::LogForked(block)) if block == chat),
        "a page checked against the head before the push was taken: {read:?}"
    );
    assert_eq!(reader.log_head(chat), Some(pushed.head));
    harness.stop().await;
}
