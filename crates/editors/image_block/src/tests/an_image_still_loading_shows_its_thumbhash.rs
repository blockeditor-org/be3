use super::*;

#[test]
fn an_image_still_loading_shows_its_thumbhash() {
    let loaded = editor(png(8, 3));
    let image = loaded.content::<ImageContent>(None);
    let derived = image
        .derived_metadata()
        .thumbhash
        .expect("decoding an image records its thumbhash in the header");
    assert_eq!((derived.width, derived.height), (8, 3));
    let shown = loaded.rect_of("image.picture");

    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let mut loading: BeuiTest<ImageApp> = BeuiTest::new(Editor::new(host, block));
    let mut info = BlockInfo::new(block, ImageContent::CONTENT_TYPE, BlockParent::Root);
    info.thumbhash = Some(derived);
    loading.store().add_block(info);
    loading.run();
    loading.run();

    assert!(
        !loading.store().holds(None),
        "the content has not arrived yet"
    );
    assert_eq!(
        loading.rect_of("image.picture"),
        shown,
        "the placeholder takes the place the image will"
    );
    loading.snapshot("an_image_still_loading_shows_its_thumbhash");
}
