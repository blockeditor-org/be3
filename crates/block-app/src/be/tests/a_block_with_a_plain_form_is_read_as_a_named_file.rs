use super::*;

fn read(block: Uuid) -> Result<be_types_host::PlainFile, String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    plain_file(block, move |read| {
        let _ = sender.send(read);
    });
    receiver.recv_timeout(PATIENCE).expect("the read answers")
}

#[test]
fn a_block_with_a_plain_form_is_read_as_a_named_file() {
    let harness = Harness::start();
    harness.connect();
    let notes = Uuid::new_v4();
    let mut named = be_block::BlockMetadata::default();
    named.insert(&be_block::NAME, &"Trip notes".to_owned());
    create(
        notes,
        text_block_types::TextContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        named,
        Some(text_block_types::TextBlock::of("# Day one\nSand.").encode()),
    );
    let file = read(notes).expect("a text block reads as a file");
    assert_eq!(file.name.as_deref(), Some("Trip notes.md"));
    assert_eq!(file.media_type, "text/markdown");
    assert_eq!(file.bytes, b"# Day one\nSand.");

    let photo = Uuid::new_v4();
    create(
        photo,
        image_block_types::ImageContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        be_block::BlockMetadata::default(),
        Some(image_block_types::ImageFile::from_file("beach.jpg", vec![0xff, 0xd8, 0xff]).encode()),
    );
    let file = read(photo).expect("an image reads as a file");
    assert_eq!(
        file.name.as_deref(),
        Some("beach.jpg"),
        "an image keeps the file it came from"
    );
    assert_eq!(file.media_type, "image/jpeg");
    assert_eq!(file.bytes, vec![0xff, 0xd8, 0xff]);

    let folder = Uuid::new_v4();
    create(
        folder,
        workspace_ui_types::FolderContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        be_block::BlockMetadata::default(),
        None,
    );
    assert!(read(folder).is_err(), "a folder has no plain-file form");
}
