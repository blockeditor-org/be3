use super::*;

use be_block::{ARTIFACT, Artifact, BlockMetadata};

fn described(source: Uuid) -> BlockMetadata {
    let mut metadata = BlockMetadata::named("Export");
    metadata.insert(
        &ARTIFACT,
        &Artifact {
            source: source.into(),
            source_type: ImageContent::CONTENT_TYPE,
            settings: Vec::new(),
        },
    );
    metadata
}

async fn backrefs(peer: &Peer<MemoryStore>, block: Uuid) -> Vec<Uuid> {
    let mut found: Vec<Uuid> = peer
        .backrefs(block)
        .await
        .unwrap()
        .into_iter()
        .map(|summary| summary.id)
        .collect();
    found.sort_unstable();
    found
}

#[tokio::test]
async fn an_artifact_is_a_backref_of_the_block_it_describes() {
    let harness = Harness::start().await;
    let author = harness.owner("exporter@example.com").await;
    let source = author
        .create::<ImageContent>(BlockParent::Root)
        .await
        .unwrap();
    let created = author
        .create_block(
            Uuid::now_v7(),
            ImageContent::CONTENT_TYPE,
            BlockParent::Root,
            &described(source),
        )
        .await
        .unwrap()
        .id;
    assert_eq!(backrefs(&author, source).await, vec![created]);

    let defined = author
        .create::<ImageContent>(BlockParent::Root)
        .await
        .unwrap();
    author
        .define(defined, |metadata| *metadata = described(source))
        .await
        .unwrap();
    let mut both = vec![created, defined];
    both.sort_unstable();
    assert_eq!(backrefs(&author, source).await, both);

    let head = author
        .save(defined, &image("export", 64, 1), None)
        .await
        .unwrap()
        .published()
        .unwrap();
    assert_eq!(backrefs(&author, source).await, both);
    assert_eq!(
        author.load_commit(head).await.unwrap().references,
        vec![source]
    );

    author
        .define(defined, |metadata| metadata.remove(&ARTIFACT))
        .await
        .unwrap();
    assert_eq!(backrefs(&author, source).await, vec![created]);
    author
        .define(created, |metadata| metadata.remove(&ARTIFACT))
        .await
        .unwrap();
    assert!(backrefs(&author, source).await.is_empty());

    harness.stop().await;
}
