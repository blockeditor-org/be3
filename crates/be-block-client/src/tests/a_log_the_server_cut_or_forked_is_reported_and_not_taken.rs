use super::*;

/// A device keeps the head of each log it verified, and takes a later head only with a proof
/// that it extends the kept one. A server that serves the log shorter than that head (cut it),
/// or a log of the same length or longer with other entries (rewrote or forked it), is
/// reported, and nothing of what it served is kept.
#[tokio::test]
async fn a_log_the_server_cut_or_forked_is_reported_and_not_taken() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let chat = workspace(&owner, "Chat").await;
    for text in ["one", "two", "three"] {
        owner.append(chat, text.as_bytes()).await.unwrap();
    }
    let kept = owner.read_log(chat, 0, 10).await.unwrap();
    assert_eq!(kept.head.size, 3);
    let past = owner.read_log(chat, 10, 5).await.unwrap();
    assert_eq!((past.head, past.entries.len()), (kept.head, 0), "a read past the end is empty");
    // A push told late, of a head as long as the kept one with another root, is a fork too.
    let told = be_block_protocol::Appended {
        organization: owner.organization(),
        block: chat,
        previous: 2,
        head: be_block_protocol::LogHead {
            size: 3,
            root: be_block_store::Hash::of(b"another root"),
        },
        entries: Vec::new(),
        proof: Vec::new(),
        consistency: Vec::new(),
    };
    assert!(
        matches!(owner.logged(told).await, Err(ClientError::LogForked(block)) if block == chat),
        "a push of another head as long as the kept one was let pass"
    );

    // The server keeps only the first two entries, made anew.
    let rewrite = |count: usize| {
        let database = harness.database();
        be_server::log::forget(&database, owner.organization(), chat).unwrap();
        let entries = (0..count)
            .map(|index| be_block_protocol::NewEntry {
                id: Uuid::new_v4(),
                payload: format!("made up {index}").into_bytes(),
                signature: vec![1; 64],
            })
            .collect();
        be_server::log::append(
            &database,
            owner.organization(),
            chat,
            owner.account(),
            entries,
            be_block_commit::now_milliseconds(),
        )
        .unwrap();
    };
    rewrite(2);
    assert!(
        matches!(owner.read_log(chat, 0, 10).await, Err(ClientError::LogForked(block)) if block == chat),
        "a log cut short was taken"
    );
    rewrite(5);
    assert!(
        matches!(owner.read_log(chat, 3, 10).await, Err(ClientError::LogForked(block)) if block == chat),
        "a forked log was taken"
    );
    assert_eq!(
        owner.log_head(chat),
        Some(kept.head),
        "the head kept moved to one that does not extend it"
    );
    harness.stop().await;
}
