use super::*;

#[tokio::test]
async fn a_block_linked_before_it_exists_is_announced_only_once_it_does() {
    let harness = Harness::start().await;
    let mut author = harness.client().await;
    let account = author.register("author@example.com").await;
    let organization = author.organization("notes").await;
    let mut other = harness
        .second_connection(&author.token.clone(), organization)
        .await;
    let writer = Author::new(account);
    other.list(BlockParent::Root).await;

    let folder = author.create_block(BlockParent::Root).await;
    let Notice::BlockChanged { block: created } = other.graph_change().await else {
        panic!("the folder was not announced");
    };
    other.list(BlockParent::Block(folder)).await;
    let note = Uuid::new_v4();
    let (commit, chunks) = writer.compose(b"lists the note", 1_000, None);
    author
        .upload(&writer.store, &writer.objects_of(commit, &chunks))
        .await;
    let response = author
        .call(|peer| {
            client::blocks::publish(
                peer,
                folder,
                commit,
                None,
                Vec::new(),
                chunks,
                1_000,
                false,
                vec![note],
                Vec::new(),
                Vec::new(),
            )
        })
        .await;
    assert!(
        matches!(response, Ok(be_protocol::PublishOutcome::Published(_))),
        "{response:?}"
    );
    let key = Uuid::new_v4();
    let keys = KeySet {
        keys: vec![row(key, note, KeyKind::Data)],
        wrapped: Vec::new(),
    };
    author
        .create_keyed(note, BlockParent::Block(folder), key, keys)
        .await;

    let mut newest = created.revision;
    loop {
        let (block, revision, present) = match other.graph_change().await {
            Notice::BlockChanged { block } => (block.id, block.revision, true),
            Notice::BlockRemoved { block, revision } => (block, revision, false),
            unexpected => panic!("not a graph change: {unexpected:?}"),
        };
        assert!(
            revision >= newest,
            "an announcement was older than the one before it"
        );
        newest = revision;
        if block == note {
            assert!(
                present,
                "the note was announced removed before it was ever made"
            );
            break;
        }
    }
    assert!(author.read_block(note).await.revision > newest);

    harness.stop().await;
}
