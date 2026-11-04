use super::*;

#[tokio::test]
async fn a_large_image_streams_without_downloading_all_of_it() {
    let harness = Harness::start().await;
    let author = harness.owner("archivist@example.com").await;
    let block = author
        .create::<ImageContent>(BlockParent::Root)
        .await
        .unwrap();

    let original = image("panorama.png", 512 * 1024, 17);
    author.save(block, &original, None).await.unwrap();
    let manifest = author.manifest(block).await.unwrap().unwrap();
    assert!(
        manifest.chunks.len() > 100,
        "the payload was stored as {} chunks",
        manifest.chunks.len()
    );

    let reader = harness.second(&author).await;
    let header = reader
        .stream_header::<ImageContent>(block)
        .await
        .unwrap()
        .expect("the header is readable");
    assert_eq!(header.source_name, "panorama.png");
    assert_eq!(header.size(), Some((1920, 1080)));
    let held = reader.commits().vault().store().count();
    assert!(
        held < 8,
        "reading a header pulled down {held} objects of {} chunks",
        manifest.chunks.len()
    );
    let after_header = reader.commits().vault().store().total_bytes();

    let slice = reader
        .stream_range::<ImageContent>(block, 300_000, 512)
        .await
        .unwrap();
    assert_eq!(slice, original.data()[300_000..300_512]);
    let fetched = reader.commits().vault().store().total_bytes() - after_header;
    assert!(
        fetched < 4_096,
        "reading 512 bytes pulled down {fetched} bytes of payload"
    );

    let tail = reader
        .stream_range::<ImageContent>(block, (original.data().len() - 8) as u64, 64)
        .await
        .unwrap();
    assert_eq!(tail, original.data()[original.data().len() - 8..]);

    let whole = reader.open::<ImageContent>(block).await.unwrap().unwrap();
    assert_eq!(whole.data(), original.data());

    harness.stop().await;
}
