use super::*;

#[tokio::test]
async fn an_image_round_trips_without_being_re_encoded() {
    let harness = Harness::start().await;
    let author = harness.owner("photographer@example.com").await;
    let block = author
        .create::<ImageContent>(BlockParent::Root)
        .await
        .unwrap();
    assert!(author.open::<ImageContent>(block).await.unwrap().is_none());

    let original = image("sunset.png", 40_000, 3);
    let saved = author.save(block, &original, None).await.unwrap();
    let head = saved.published().expect("the first save publishes");
    assert_eq!(saved, Saved::Published(head));

    let encoded = original.encode();
    assert!(
        encoded.ends_with(original.data()),
        "the pixel bytes were re-encoded on the way into storage"
    );

    let reader = harness.second(&author).await;
    let loaded = reader
        .open::<ImageContent>(block)
        .await
        .unwrap()
        .expect("the image is readable");
    assert_eq!(loaded, original);
    assert_eq!(loaded.name().as_deref(), Some("sunset.png"));
    assert_eq!(loaded.header().size(), Some((1920, 1080)));

    let unchanged = author.save(block, &original, Some(head)).await.unwrap();
    assert_eq!(unchanged, Saved::Unchanged(head));
    assert_eq!(author.history(block).await.unwrap().len(), 1);

    let mut replaced = original.clone();
    replaced.set_header(ImageHeader {
        source_name: "sunset-cropped.png".into(),
        media_type: "image/png".into(),
        width: 960,
        height: 540,
        failure: None,
    });
    let second = author.save(block, &replaced, Some(head)).await.unwrap();
    assert!(matches!(second, Saved::Published(_)));
    assert_eq!(
        reader.open::<ImageContent>(block).await.unwrap().unwrap(),
        replaced
    );

    harness.stop().await;
}
