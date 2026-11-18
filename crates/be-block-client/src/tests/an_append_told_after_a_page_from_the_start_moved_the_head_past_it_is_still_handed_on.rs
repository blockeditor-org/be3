use super::*;

/// A page read from a long log's start takes the head the server has then, which may already
/// hold an entry whose push is still on its way. That push, told late, still hands the entry on
/// once it proves it, so a replica that read old history while the entry landed hears it. What
/// it hands on is proved against the head this device kept, never against the head the push
/// names alone: a server that tells of a made-up shorter head hands on nothing made up.
#[tokio::test]
async fn an_append_told_after_a_page_from_the_start_moved_the_head_past_it_is_still_handed_on() {
    const ENTRIES: u64 = 300;
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let chat = workspace(&owner, "Chat").await;
    let database = harness.database();
    be_server::log::append(
        &database,
        owner.organization(),
        chat,
        owner.account(),
        (0..ENTRIES)
            .map(|index| be_block_protocol::NewEntry {
                id: Uuid::from_u64_pair(0, index),
                payload: index.to_be_bytes().to_vec(),
                signature: vec![1; 64],
            })
            .collect(),
        1_000,
    )
    .unwrap();
    drop(database);
    let reader = harness.second(&owner).await;
    let mut events = reader.connection().subscribe();
    let first = reader.read_log(chat, 0, 10).await.unwrap();
    assert_eq!((first.head.size, first.entries.len()), (ENTRIES, 10));

    owner.append(chat, "test.app", b"late").await.unwrap();
    let pushed = loop {
        if let Ok(crate::Event::Appended(appended)) = events.recv().await {
            break appended;
        }
    };
    // Another page from the start answers before the push is taken: its head holds the entry.
    let again = reader.read_log(chat, 0, 10).await.unwrap();
    assert_eq!(again.head.size, ENTRIES + 1);

    let heard = reader.logged(pushed).await.unwrap();
    assert_eq!(
        heard
            .iter()
            .map(|logged| (logged.entry.seq, logged.bytes().map(<[u8]>::to_vec)))
            .collect::<Vec<_>>(),
        [(ENTRIES, Some(b"late".to_vec()))],
        "the push told late still hands on its entry"
    );

    // A push of a made-up head shorter than the kept one, of an entry never in the log at a
    // place this device never read, proves only against its own head.
    let mut made_up = heard[0].entry.clone();
    let mut tree = be_block_keys::merkle::Tree::new();
    for seq in 0..=100 {
        made_up.seq = seq;
        made_up.id = Uuid::new_v4();
        tree.push(made_up.leaf());
    }
    let forged = be_block_protocol::Appended {
        organization: owner.organization(),
        block: chat,
        previous: 100,
        head: be_block_protocol::LogHead {
            size: 101,
            root: be_block_store::Hash::from_bytes(tree.root()),
        },
        entries: vec![made_up.clone()],
        proof: tree
            .range_proof(101, 100, 101)
            .into_iter()
            .map(be_block_store::Hash::from_bytes)
            .collect(),
        consistency: Vec::new(),
    };
    let handed = reader.logged(forged).await.unwrap();
    assert_eq!(
        handed
            .iter()
            .map(|logged| (logged.entry.seq, logged.entry.id))
            .collect::<Vec<_>>(),
        [(100, Uuid::from_u64_pair(0, 100))],
        "what is handed on is the log's own entry at that place"
    );
    harness.stop().await;
}
