use super::*;
use be_type_text::{TextBlock, TextContent};

#[tokio::test]
async fn objects_move_through_the_servers_own_s3_endpoint() {
    let harness = Harness::start().await;
    let phone = device(&harness, "phone@example.com").await;
    let note = note(&phone).await;
    let text = large_text();
    phone.save(note, &TextBlock::of(&text), None).await.unwrap();
    let head = phone.summary(note).await.unwrap().head.unwrap();
    let chunks = phone
        .load_commit(head)
        .await
        .unwrap()
        .manifest
        .chunk_hashes();
    let missing = crate::ask(be_block_protocol::client::objects::missing_objects(
        &phone.connection(),
        chunks.clone(),
    ))
    .await
    .unwrap();
    assert!(missing.is_empty(), "the server lacks {missing:?}");

    let laptop = another(&harness, &phone).await;
    laptop.fetch(&chunks).await.unwrap();
    let opened = laptop.open::<TextContent>(note).await.unwrap().unwrap();
    assert_eq!(TextBlock::text(&opened), text);
}
