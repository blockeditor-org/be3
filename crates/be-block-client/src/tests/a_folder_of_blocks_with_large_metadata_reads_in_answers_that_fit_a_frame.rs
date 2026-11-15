use super::*;
use be_block_content::BlockMetadata;
use be_block_protocol::MAX_FRAME_BYTES;
use be_server::limits::MAX_METADATA_BYTES;
use be_type_text::TextContent;

#[tokio::test]
async fn a_folder_of_blocks_with_large_metadata_reads_in_answers_that_fit_a_frame() {
    let harness = Harness::start().await;
    let author = harness.owner("archivist@example.com").await;
    let workspace = workspace(&author, "Archive").await;
    // Names near the metadata limit, and enough of them that their summaries alone pass a frame.
    let name_bytes = MAX_METADATA_BYTES - 1024;
    let count = MAX_FRAME_BYTES / name_bytes + 2;
    let mut wanted = Vec::new();
    for index in 0..count {
        let name = format!("{index:>name_bytes$}");
        let created = author
            .create_block(
                Uuid::now_v7(),
                TextContent::CONTENT_TYPE,
                BlockParent::Block(workspace),
                &BlockMetadata::named(&name),
                false,
            )
            .await
            .unwrap();
        assert!(created.metadata.len() > name_bytes);
        wanted.push(created.id);
    }

    // Listing the folder answers the first of them and the rest as ids, which the peer reads.
    let listed = crate::ask(be_block_protocol::client::blocks::list_blocks(
        &author.connection(),
        Listing::Children(BlockParent::Block(workspace)),
        None,
    ))
    .await
    .unwrap();
    assert!(!listed.blocks.is_empty() && !listed.rest.is_empty());
    assert!(be_block_protocol::encoded_len(&listed) < MAX_FRAME_BYTES);
    let opened = harness.second(&author).await;
    let children = opened
        .list(Listing::Children(BlockParent::Block(workspace)))
        .await
        .unwrap();
    let mut listed: Vec<Uuid> = children.iter().map(|summary| summary.id).collect();
    let mut made = wanted.clone();
    listed.sort();
    made.sort();
    assert_eq!(listed, made);
    assert!(
        children
            .iter()
            .all(|summary| summary.metadata.len() > name_bytes)
    );

    // One call asking for every block is answered with the first of them, as many as fit.
    let answer = crate::ask(be_block_protocol::client::blocks::read_contents(
        &author.connection(),
        wanted.clone(),
        Vec::new(),
    ))
    .await
    .unwrap();
    assert!(!answer.blocks.is_empty() && answer.blocks.len() < count);
    assert!(be_block_protocol::encoded_len(&answer) < MAX_FRAME_BYTES);
    assert!(
        answer
            .blocks
            .iter()
            .zip(&wanted)
            .all(|(summary, id)| summary.id == *id)
    );

    // The peer asks again for the rest, and reads every block in order.
    let reader = harness.second(&author).await;
    let asked: Vec<_> = wanted.iter().map(|id| (*id, None)).collect();
    let read = reader.read_contents(&asked).await.unwrap();
    assert_eq!(
        read.iter().map(|read| read.summary.id).collect::<Vec<_>>(),
        wanted
    );
    assert!(
        read.iter()
            .all(|read| read.summary.metadata.len() > name_bytes)
    );

    // What changed in the workspace's tree comes in pages that fit too.
    let (changed, _, _) = reader.changed_since(workspace, 0).await.unwrap();
    for id in &wanted {
        assert!(changed.iter().any(|summary| summary.id == *id));
    }

    harness.stop().await;
}
