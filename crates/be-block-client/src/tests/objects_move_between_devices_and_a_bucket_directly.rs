use super::*;
use be_server::TestBucket;
use be_type_text::{TextBlock, TextContent};

#[tokio::test]
async fn objects_move_between_devices_and_a_bucket_directly() {
    let bucket = TestBucket::start();
    let harness = Harness::configured(be_server::ServerConfig {
        bucket: Some(bucket.config(64 << 20)),
        ..be_server::ServerConfig::OPEN
    })
    .await;
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
    assert!(chunks.len() > 1);
    for chunk in &chunks {
        assert!(bucket.file(*chunk).exists(), "{chunk} is not in the bucket");
    }
    let passed = std::fs::read_dir(harness.server.directory.join("object-cache"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(passed, 0, "the bytes passed through the server");

    // Another device reads them from the bucket, checking each against its hash.
    let laptop = another(&harness, &phone).await;
    laptop.fetch(&chunks).await.unwrap();
    let opened = laptop.open::<TextContent>(note).await.unwrap().unwrap();
    assert_eq!(TextBlock::text(&opened), text);

    let tampered = chunks[0];
    std::fs::write(bucket.file(tampered), b"not the object").unwrap();
    let tablet = another(&harness, &phone).await;
    let fetched = tablet.fetch(&[tampered]).await;
    assert!(
        matches!(fetched, Err(ClientError::Tampered(hash)) if hash == tampered),
        "{fetched:?}"
    );
}
