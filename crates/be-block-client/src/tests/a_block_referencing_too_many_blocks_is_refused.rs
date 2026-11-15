use super::*;
use be_block_content::Entries;
use be_block_protocol::{ErrorCode, KeySet, MAX_REFERENCES, client::blocks};
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
        crate::ask(blocks::set_metadata(
            &author.connection(),
            block,
            Vec::new(),
            too_many.clone(),
        ))
        .await,
    );
    refused(
        crate::ask(blocks::create_block(
            &author.connection(),
            Uuid::now_v7(),
            TextContent::CONTENT_TYPE,
            BlockParent::Block(workspace),
            Vec::new(),
            too_many.clone(),
            Uuid::new_v4(),
            KeySet::default(),
            false,
        ))
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
    let kept = crate::ask(blocks::set_metadata(
        &author.connection(),
        block,
        Vec::new(),
        most,
    ))
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
    let summaries = crate::ask(blocks::read_summaries(
        &reader.connection(),
        vec![block, sibling],
    ))
    .await
    .unwrap();
    assert!(!summaries.is_empty());
    assert_eq!(
        reader.summary(block).await.unwrap().references.len(),
        MAX_REFERENCES
    );

    harness.stop().await;
}
