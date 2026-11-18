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
