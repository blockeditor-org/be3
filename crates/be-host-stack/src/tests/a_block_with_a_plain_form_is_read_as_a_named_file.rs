use super::*;

fn read(block: Uuid) -> Result<be_block_type_host::PlainFile, String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    plain_file(block, move |read| {
        let _ = sender.send(read);
    });
    receiver.recv().expect("the read answers")
}

#[test]
fn a_block_with_a_plain_form_is_read_as_a_named_file() {
    let harness = Harness::start();
    harness.connect();
    let notes = Uuid::new_v4();
    let mut named = be_block_content::BlockMetadata::default();
    named.insert(&be_block_content::NAME, &"Trip notes".to_owned());
    create(
        notes,
        be_type_text::TextContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        named,
        Some(be_type_text::TextBlock::of("# Day one\nSand.").encode()),
        false,
    );
    let file = read(notes).expect("a text block reads as a file");
    assert_eq!(file.name.as_deref(), Some("Trip notes.md"));
    assert_eq!(file.media_type, "text/markdown");
    assert_eq!(file.bytes, b"# Day one\nSand.");

    let photo = Uuid::new_v4();
    create(
        photo,
        be_type_image::ImageContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::default(),
        Some(be_type_image::ImageFile::from_file("beach.jpg", vec![0xff, 0xd8, 0xff]).encode()),
        false,
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
        be_type_workspace::FolderContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    assert!(read(folder).is_err(), "a folder has no plain-file form");
}
