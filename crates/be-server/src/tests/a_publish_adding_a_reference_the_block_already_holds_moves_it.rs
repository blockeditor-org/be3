use be_block_protocol::{PlacedReference, ReferenceChange};

use super::*;

async fn publish_references(
    client: &mut TestClient,
    author: &Author,
    block: Uuid,
    text: &[u8],
    time: i64,
    expected: Option<CommitId>,
    references: ReferenceChange,
) -> Result<be_block_protocol::PublishOutcome, Error> {
    let (commit, chunks) = author.compose(text, time, expected);
    client
        .upload(&author.store, &author.objects_of(commit, &chunks))
        .await;
    let before = author.held_by(expected);
    client
        .call(|peer| {
            client::blocks::publish(
                peer,
                block,
                commit,
                expected,
                expected.into_iter().collect(),
                objects_change(&before, chunks),
                time,
                false,
                references,
                Vec::new(),
                Vec::new(),
            )
        })
        .await
}

/// A client diffs references against its own view of the list, which may already hold one it
/// adds (one it could not resolve yet, or one another device's commit put there): the publish
/// is kept, and the reference moves to where it was added.
#[tokio::test]
async fn a_publish_adding_a_reference_the_block_already_holds_moves_it() {
    let harness = Harness::start().await;
    let mut client = harness.client().await;
    let account = client.register("linker@example.com").await;
    client.organization("links").await;
    let author = Author::new(account);
    let view = client.create_block(BlockParent::Root).await;
    let [a, b, c] = [
        client.create_block(BlockParent::Root).await,
        client.create_block(BlockParent::Root).await,
        client.create_block(BlockParent::Root).await,
    ];

    let first = publish_references(
        &mut client,
        &author,
        view,
        b"first",
        1_000,
        None,
        listing(&[], vec![a, b]),
    )
    .await;
    let Ok(be_block_protocol::PublishOutcome::Published(head)) = first else {
        panic!("{first:?}");
    };
    assert_eq!(client.read_block(view).await.references, vec![a, b]);

    // `b` is added at the front though the block holds it, and `c` at the end.
    let readding = ReferenceChange {
        removed: Vec::new(),
        added: vec![
            PlacedReference { index: 0, block: b },
            PlacedReference { index: 2, block: c },
        ],
    };
    let second = publish_references(
        &mut client,
        &author,
        view,
        b"second",
        2_000,
        Some(head),
        readding,
    )
    .await;
    assert!(
        matches!(second, Ok(be_block_protocol::PublishOutcome::Published(_))),
        "{second:?}"
    );
    assert_eq!(client.read_block(view).await.references, vec![b, a, c]);

    harness.stop().await;
}
