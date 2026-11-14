use super::*;
use be_block_protocol::{ErrorCode, KeySet, client::blocks};
use be_server::limits::MAX_REFERENCES;
use be_type_text::TextContent;

#[tokio::test]
async fn a_block_referencing_too_many_blocks_is_refused() {
    let harness = Harness::start().await;
    let author = harness.owner("collector@example.com").await;
    let workspace = workspace(&author, "Links").await;
    let block = author
        .create::<TextContent>(BlockParent::Block(workspace))
        .await
        .unwrap();
    let too_many: Vec<Uuid> = (0..=MAX_REFERENCES).map(|_| Uuid::new_v4()).collect();
    let refused = |answer: Result<_, ClientError>| match answer {
        Err(ClientError::Refused(ErrorCode::InvalidRequest, _)) => {}
        Err(error) => panic!("refused otherwise: {error:?}"),
        Ok(_) => panic!("the references were kept"),
    };

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
            too_many,
            Uuid::new_v4(),
            KeySet::default(),
            false,
        ))
        .await,
    );

    harness.stop().await;
}
