use super::*;

#[test]
fn an_image_still_loading_shows_its_thumbhash() {
    let loaded = editor(png(8, 4));
    let thumbhash = loaded
        .content::<ImageContent>(None)
        .header()
        .thumbhash
        .clone();
    assert!(
        thumbhash.is_some(),
        "decoding an image records its thumbhash in the header"
    );

    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let mut loading: ContentHarness<ImageApp> =
        ContentHarness::new(BeuiTest::new(Editor::new(host.clone(), block)), host);
    let mut info = BlockInfo::new(block, ImageContent::CONTENT_TYPE, BlockParent::Root);
    info.thumbhash = thumbhash;
    loading.store().add_block(info);
    loading.run();
    loading.run();

    assert!(
        !loading.store().holds(None),
        "the content has not arrived yet"
    );
    let rect = loading.rect_of("image.picture");
    assert!(
        rect.width() > rect.height(),
        "the placeholder takes the image's shape before the image arrives"
    );
    loading.snapshot("an_image_still_loading_shows_its_thumbhash");
}
