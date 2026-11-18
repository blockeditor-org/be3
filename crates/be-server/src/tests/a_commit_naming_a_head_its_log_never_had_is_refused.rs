use super::*;

/// A commit may name the head of its block's signed log it was made on, and the server takes it
/// only if that is a head the log had: a size past the log, or a root that is not the root of
/// the log's first entries, is refused, and a head the log had, however far back, is taken.
#[tokio::test]
async fn a_commit_naming_a_head_its_log_never_had_is_refused() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let owner_id = owner.id();
    owner.personal().await;
    let chat = owner.create_block(BlockParent::Root).await;
    owner.append(chat, vec![log_entry(b"one")]).await.unwrap();
    let one = owner.log_page(chat, 0, None, false).await.unwrap().head;
    owner
        .append(chat, vec![log_entry(b"two"), log_entry(b"three")])
        .await
        .unwrap();
    let three = owner.log_page(chat, 0, None, false).await.unwrap().head;

    let author = Author::new(owner_id);
    let (head, chunks) = author.compose(b"settings\n", 1_000, None);
    owner
        .upload(&author.store, &author.objects_of(head, &chunks))
        .await;
    let past = publish(
        &mut owner,
        chat,
        head,
        &chunks,
        be_block_protocol::LogHead {
            size: 4,
            root: three.root,
        },
    )
    .await;
    assert!(
        matches!(&past, Err(error) if error.code == ErrorCode::InvalidRequest),
        "a head past the log was taken: {past:?}"
    );
    let wrong = publish(
        &mut owner,
        chat,
        head,
        &chunks,
        be_block_protocol::LogHead {
            size: 1,
            root: three.root,
        },
    )
    .await;
    assert!(
        matches!(&wrong, Err(error) if error.code == ErrorCode::InvalidRequest),
        "a head with another root was taken: {wrong:?}"
    );
    let earlier = publish(&mut owner, chat, head, &chunks, one).await;
    assert_eq!(
        earlier,
        Ok(be_block_protocol::PublishOutcome::Published(head)),
        "a head the log had was refused"
    );
    harness.stop().await;
}

/// Publishes `head` as the block's first commit, naming `through` as its log's head.
async fn publish(
    owner: &mut TestClient,
    chat: Uuid,
    head: CommitId,
    chunks: &[Hash],
    through: be_block_protocol::LogHead,
) -> Result<be_block_protocol::PublishOutcome, Error> {
    let chunks = chunks.to_vec();
    owner
        .call(move |peer| {
            peer.write::<be_block_protocol::PublishOutcome>(Op::Publish {
                block: chat,
                expected: None,
                commit: Some(be_block_protocol::NewCommit {
                    id: head,
                    parents: Vec::new(),
                    held: objects_change(&[], chunks),
                    time: 1_000,
                    pinned: false,
                    objects: Vec::new(),
                    log_through: Some(through),
                }),
                references: listing(&[], Vec::new()),
                metadata: Vec::new(),
                journal: None,
            })
        })
        .await
}
