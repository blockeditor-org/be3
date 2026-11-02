use super::*;

use crate::transfer::{self, Saved, renumber};
use be_block::{ARTIFACT, Artifact, BlockMetadata};

#[tokio::test]
async fn an_exported_artifact_imports_pointing_to_its_imported_source() {
    let harness = Harness::start().await;
    let author = harness.owner("importer@example.com").await;
    let folder = Uuid::now_v7();
    let source = Uuid::now_v7();
    let artifact = Uuid::now_v7();
    let mut metadata = BlockMetadata::named("Export");
    metadata.insert(
        &ARTIFACT,
        &Artifact {
            source: source.into(),
            source_type: ImageContent::CONTENT_TYPE,
            settings: b"{}".to_vec(),
        },
    );
    let saved = |id, parent, metadata| Saved {
        id,
        block_type: ImageContent::CONTENT_TYPE,
        parent,
        metadata,
        references: Vec::new(),
        content: Some(image("export", 64, 2).encode()),
    };
    let blocks = vec![
        saved(folder, None, BlockMetadata::named("Folder")),
        saved(source, Some(0), BlockMetadata::named("Source")),
        saved(artifact, Some(0), metadata),
    ];
    let blocks = transfer::decode(&transfer::encode(&blocks)).unwrap();

    let blocks = renumber(blocks);
    let (new_source, new_artifact) = (blocks[1].id, blocks[2].id);
    assert_ne!(new_source, source);
    let pointed = blocks[2].metadata.get(&ARTIFACT).unwrap();
    assert_eq!(
        *pointed.source, new_source,
        "renumbering kept the old source"
    );
    assert_eq!(pointed.settings, b"{}");

    let placed: Vec<_> = blocks
        .iter()
        .map(|block| {
            let parent = match block.parent {
                Some(position) => BlockParent::Block(blocks[position].id),
                None => BlockParent::Root,
            };
            (block.clone(), parent)
        })
        .collect();
    let mut failed = Vec::new();
    author
        .import_blocks(&placed, |block, imported| {
            if let Err(error) = imported {
                failed.push((block, error.to_string()));
            }
        })
        .await;
    assert!(failed.is_empty(), "{failed:?}");

    let backrefs: Vec<Uuid> = author
        .backrefs(new_source)
        .await
        .unwrap()
        .into_iter()
        .map(|summary| summary.id)
        .collect();
    assert_eq!(backrefs, vec![new_artifact]);
    let summary = author.summary(new_artifact).await.unwrap();
    let imported = author.metadata_of_summary(&summary).await.unwrap();
    assert_eq!(*imported.get(&ARTIFACT).unwrap().source, new_source);

    harness.stop().await;
}
