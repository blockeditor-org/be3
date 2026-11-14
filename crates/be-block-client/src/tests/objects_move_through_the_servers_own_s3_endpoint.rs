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
    let manifest = phone.load_commit(head).await.unwrap().manifest;
    let chunks = phone.objects_of(&manifest).await.unwrap();
    // Every object the save sent: the server holds them, so it asks for none again (their
    // lengths, which only room for an upload is reserved by, do not matter).
    let lengths = chunks
        .iter()
        .map(|hash| be_block_protocol::ObjectLength {
            hash: *hash,
            length: Some(1),
        })
        .collect();
    let urls = crate::ask(be_block_protocol::client::objects::upload_urls(
        &phone.connection(),
        lengths,
    ))
    .await
    .unwrap();
    assert!(
        urls.iter().all(Option::is_none),
        "the server lacks some of them"
    );

    let laptop = another(&harness, &phone).await;
    laptop.fetch(&chunks).await.unwrap();
    let opened = laptop.open::<TextContent>(note).await.unwrap().unwrap();
    assert_eq!(TextBlock::text(&opened), text);
}
