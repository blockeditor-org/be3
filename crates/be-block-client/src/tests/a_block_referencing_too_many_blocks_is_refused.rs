use super::*;
use be_block_content::Entries;
use be_block_protocol::{ErrorCode, KeySet, MAX_REFERENCES};
use be_type_text::TextContent;

fn refused<T>(answer: Result<T, ClientError>) {
    match answer {
        Err(ClientError::Refused(ErrorCode::InvalidRequest, _)) => {}
        Err(error) => panic!("refused otherwise: {error:?}"),
        Ok(_) => panic!("the references were kept"),
    }
}

/// A block references at most `MAX_REFERENCES` blocks. One that does is summarized alone in
/// any answer, which still fits a frame; one more is refused by the server, and by the client
/// before anything is put in the outbox.
#[tokio::test]
async fn a_block_referencing_too_many_blocks_is_refused() {
    let harness = Harness::start().await;
    let author = harness.owner("collector@example.com").await;
    let workspace = workspace(&author, "Links").await;
    let block = author
        .create::<TextContent>(BlockParent::Block(workspace))
        .await
        .unwrap();
    let sibling = author
        .create::<TextContent>(BlockParent::Block(workspace))
        .await
        .unwrap();
    let most: Vec<Uuid> = (0..MAX_REFERENCES).map(|_| Uuid::new_v4()).collect();
    let mut too_many = most.clone();
    too_many.push(Uuid::new_v4());

    refused(
        crate::ask(be_block_protocol::write::<be_block_protocol::BlockSummary, _>(&author.connection(), Some(author.organization()), be_block_protocol::Op::SetMetadata { block: block, metadata: Vec::new(), references: too_many.clone() }))
        .await,
    );
    refused(
        crate::ask(be_block_protocol::write::<be_block_protocol::BlockSummary, _>(&author.connection(), Some(author.organization()), be_block_protocol::Op::CreateBlock { block: Uuid::now_v7(), content_type: TextContent::CONTENT_TYPE, parent: BlockParent::Block(workspace), metadata: Vec::new(), references: too_many.clone(), key: Uuid::new_v4(), keys: KeySet::default(), owner_only: false, root: Vec::new(), placement: Vec::new() }))
        .await,
    );
    refused(
        author
            .write(
                sibling,
                Write::encoded(TextContent::CONTENT_TYPE, Vec::<u8>::new(), None)
                    .described(too_many, Entries::default()),
            )
            .await,
    );
    assert!(
        author.pending_commits(sibling).is_empty(),
        "a write that could never be sent was kept"
    );

    // As many as may be: the block's summary passes every answer alone.
    let kept = crate::ask(be_block_protocol::write::<be_block_protocol::BlockSummary, _>(&author.connection(), Some(author.organization()), be_block_protocol::Op::SetMetadata { block: block, metadata: Vec::new(), references: most }))
    .await
    .unwrap();
    assert_eq!(kept.references.len(), MAX_REFERENCES);
    let reader = harness.second(&author).await;
    let listed = reader
        .list(Listing::Children(BlockParent::Block(workspace)))
        .await
        .unwrap();
    assert!(
        listed
            .iter()
            .any(|summary| summary.id == block && summary.references.len() == MAX_REFERENCES)
    );
    assert!(listed.iter().any(|summary| summary.id == sibling));
    let read = reader
        .read_contents(&[(sibling, None), (block, None), (sibling, None)])
        .await
        .unwrap();
    assert_eq!(read[1].summary.references.len(), MAX_REFERENCES);
    let summaries = crate::ask(be_block_protocol::read::<Vec<Option<be_block_protocol::BlockSummary>>, _>(&reader.connection(), Some(author.organization()), be_block_protocol::Query::Blocks { blocks: vec![block, sibling], contents: false, held: Vec::new() }))
    .await
    .unwrap();
    assert!(!summaries.is_empty());
    assert_eq!(
        reader.summary(block).await.unwrap().references.len(),
        MAX_REFERENCES
    );

    harness.stop().await;
}
