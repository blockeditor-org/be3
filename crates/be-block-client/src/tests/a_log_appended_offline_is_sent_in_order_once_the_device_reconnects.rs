use super::*;

/// Entries appended offline wait in the outbox, in order, and a drain sends them in that order
/// once the device reconnects: the server numbers and stamps each, and a read verifies them
/// against the log's head and opens them, signed by this account.
#[tokio::test]
async fn a_log_appended_offline_is_sent_in_order_once_the_device_reconnects() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    let chat = workspace(&laptop, "Chat").await;
    let first = laptop
        .append(chat, "test.app", b"sent online", be_block_content::Along::default())
        .await
        .unwrap();
    assert!(
        matches!(first, Appended::Logged { seq: 0, .. }),
        "{first:?}"
    );
    drop(laptop);

    let offline = harness.offline(&owner, &refs, &store);
    let mut kept = Vec::new();
    for text in ["one", "two", "three"] {
        match offline
            .append(chat, "test.app", text.as_bytes(), be_block_content::Along::default())
            .await
            .unwrap()
        {
            Appended::Kept { id } => kept.push(id),
            logged => panic!("an entry made offline was logged: {logged:?}"),
        }
    }
    assert_eq!(offline.pending().operations, 3);
    assert_eq!(
        offline.read_log(chat, 0, 10).await.unwrap().head.size,
        0,
        "offline, the log is what this device verified, which is nothing of it yet"
    );

    harness.reconnect(&offline, owner.token()).await;
    let drained = offline.drain(|_| false).await.unwrap();
    let appended: Vec<(Uuid, u64, i64)> = drained
        .iter()
        .filter_map(|outcome| match outcome {
            Drained::Appended {
                block,
                id,
                seq,
                arrival,
            } if *block == chat => Some((*id, *seq, *arrival)),
            _ => None,
        })
        .collect();
    assert_eq!(
        appended
            .iter()
            .map(|(id, seq, _)| (*id, *seq))
            .collect::<Vec<_>>(),
        kept.iter().copied().zip(1..).collect::<Vec<_>>(),
        "the entries went in the order they were made: {drained:?}"
    );
    assert!(appended.windows(2).all(|pair| pair[0].2 <= pair[1].2));
    assert_eq!(offline.pending(), refs::PendingCounts::default());

    let read = offline.read_log(chat, 0, 10).await.unwrap();
    assert_eq!(read.head.size, 4);
    assert_eq!(
        read.entries
            .iter()
            .map(|logged| logged.bytes().unwrap().to_vec())
            .collect::<Vec<_>>(),
        [&b"sent online"[..], b"one", b"two", b"three"]
    );
    for logged in &read.entries {
        assert_eq!(logged.entry.author, owner.account());
        assert_eq!(
            offline
                .verify_entry(
                    chat,
                    "test.app",
                    logged.entry.author,
                    logged.entry.id,
                    logged.entry.arrival,
                    logged.opened.as_ref().unwrap(),
                    &logged.entry.signature,
                )
                .await
                .unwrap(),
            Signer::Verified
        );
    }

    // Another device of the account reads the same log, and a page of it from anywhere, and
    // following it, takes what is appended next as it is told, proving it extends its head.
    let phone = harness.second(&owner).await;
    let mut events = phone.connection().subscribe();
    let page = phone.read_log(chat, 2, 1).await.unwrap();
    assert_eq!(page.head, read.head);
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.entries[0].bytes(), Some(&b"two"[..]));
    offline.append(chat, "test.app", b"four", be_block_content::Along::default()).await.unwrap();
    let appended = loop {
        if let Ok(crate::Event::Appended(appended)) = events.recv().await {
            break appended;
        }
    };
    let took = phone.logged(appended).await.unwrap();
    assert_eq!(took.len(), 1);
    assert_eq!(took[0].entry.seq, 4);
    assert_eq!(took[0].bytes(), Some(&b"four"[..]));
    assert_eq!(phone.log_head(chat).map(|head| head.size), Some(5));
    harness.stop().await;
}
