use super::*;
use be_block_content::Entries;
use be_block_protocol::{ErrorCode, MAX_REFERENCES};
use be_type_text::TextContent;

fn refused<T>(answer: Result<T, ClientError>) {
    match answer {
        Err(ClientError::Refused(ErrorCode::InvalidRequest, _)) => {}
        Err(error) => panic!("refused otherwise: {error:?}"),
        Ok(_) => panic!("the references were kept"),
    }
}

/// A block references at most `MAX_REFERENCES` blocks: a publish adding that many is kept, and
/// one more is refused by the server, and by the client before anything is put in the outbox.
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
        crate::ask(
            be_block_protocol::write::<be_block_protocol::BlockSummary, _>(
                &author.connection(),
                Some(author.organization()),
                be_block_protocol::Op::Publish {
                    block: block,
                    expected: None,
                    commit: None,
                    references: be_block_protocol::ReferenceChange::between(&[], &too_many.clone()),
                    metadata: Vec::new(),
                    journal: None,
                },
            ),
        )
        .await,
    );
    refused(
        crate::ask(
            be_block_protocol::write::<be_block_protocol::BlockSummary, _>(
                &author.connection(),
                Some(author.organization()),
                be_block_protocol::Op::CreateBlock {
                    block: Uuid::now_v7(),
                    content_type: TextContent::CONTENT_TYPE,
                    parent: BlockParent::Block(workspace),
                    metadata: Vec::new(),
                    references: too_many.clone(),
                    key: Uuid::new_v4(),
                    owner_only: false,
                    root: Vec::new(),
                    placement: Vec::new(),
                },
            ),
        )
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

    // As many as may be: a publish adding them all fits a frame and is kept.
    let kept = crate::ask(
        be_block_protocol::write::<be_block_protocol::BlockSummary, _>(
            &author.connection(),
            Some(author.organization()),
            be_block_protocol::Op::Publish {
                block: block,
                expected: None,
                commit: None,
                references: be_block_protocol::ReferenceChange::between(&[], &most),
                metadata: Vec::new(),
                journal: None,
            },
        ),
    )
    .await;
    assert!(kept.is_ok(), "{kept:?}");

    harness.stop().await;
}
