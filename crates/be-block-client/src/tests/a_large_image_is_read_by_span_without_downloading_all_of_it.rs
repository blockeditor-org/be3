use super::*;

#[tokio::test]
async fn a_large_image_is_read_by_span_without_downloading_all_of_it() {
    let harness = Harness::start().await;
    let author = harness.owner("archivist@example.com").await;
    let block = author
        .create::<ImageContent>(BlockParent::Root)
        .await
        .unwrap();

    let original = image("panorama.png", 512 * 1024, 17);
    author.save(block, &original, None).await.unwrap();
    let manifest = author.manifest(block).await.unwrap().unwrap();
    let objects = author.commits().vault().objects(&manifest).unwrap().len();
    assert!(objects > 100, "the payload was stored as {objects} objects");

    let reader = harness.second(&author).await;
    let manifest = reader.manifest(block).await.unwrap().unwrap();
    let stored = reader.load_first_part(&manifest).await.unwrap();
    let loaded = ImageContent::from_stored(&stored).unwrap();
    assert!(!loaded.is_loaded(), "only the header was loaded");
    assert_eq!(loaded.header().source_name, "panorama.png");
    assert_eq!(loaded.header().size(), Some((1920, 1080)));
    assert_eq!(loaded.payload_length(), original.data().len() as u64);
    // Loading the first part fetches the manifest's nodes and the header's chunk, none of
    // the payload's chunks.
    let held = reader.commits().vault().store().count();
    assert!(
        held < objects / 16,
        "reading a header pulled down {held} objects of {objects}"
    );
    let after_header = reader.commits().vault().store().total_bytes();

    let start = loaded.payload_offset();
    let slice = reader
        .read_span(&manifest, start + 300_000, 512)
        .await
        .unwrap();
    assert_eq!(slice, original.data()[300_000..300_512]);
    let fetched = reader.commits().vault().store().total_bytes() - after_header;
    // Leaves are cut by content, so their size varies with the random keys:
    // ask for a small fraction of the payload, not a fixed number of bytes.
    let whole = original.data().len();
    assert!(
        fetched < whole / 16,
        "reading 512 bytes pulled down {fetched} bytes of payload, of {whole}"
    );

    let tail = reader
        .read_span(&manifest, start + (whole - 8) as u64, 64)
        .await
        .unwrap();
    assert_eq!(tail, original.data()[whole - 8..]);

    let opened = reader.open::<ImageContent>(block).await.unwrap().unwrap();
    assert_eq!(opened.data(), original.data());

    harness.stop().await;
}
