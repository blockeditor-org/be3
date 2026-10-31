use super::*;

fn imported(name: &str, bytes: &[u8]) -> (Uuid, Uuid) {
    let (sender, receiver) = std::sync::mpsc::channel();
    import_file(
        name.to_owned(),
        bytes.to_vec(),
        be_graph::BlockParent::Root,
        move |imported| {
            let _ = sender.send(imported);
        },
    );
    receiver
        .recv()
        .expect("the import answers")
        .expect("the import succeeds")
}

fn read(block: Uuid) -> be_types_host::PlainFile {
    let (sender, receiver) = std::sync::mpsc::channel();
    plain_file(block, move |read| {
        let _ = sender.send(read);
    });
    receiver
        .recv()
        .expect("the read answers")
        .expect("the block reads as a file")
}

#[test]
fn a_picked_file_becomes_a_block_of_the_type_that_claims_it() {
    let harness = Harness::start();
    harness.connect();

    let (photo, photo_type) = imported("beach.png", &[0x89, b'P', b'N', b'G']);
    assert_eq!(
        photo_type,
        image_block_types::ImageContent::CONTENT_TYPE,
        "an image type claims a .png, as it does for be sync"
    );
    assert_eq!(read(photo).bytes, vec![0x89, b'P', b'N', b'G']);

    let (book, book_type) = imported("Moby Dick.epub", b"PK\x03\x04\x14\x00\x00\x00book");
    assert_eq!(
        book_type,
        workspace_ui_types::FileContent::CONTENT_TYPE,
        "a file no other type claims is a file block"
    );
    assert_eq!(
        node(book).and_then(|node| node.metadata.name()),
        Some("Moby Dick.epub".to_owned()),
        "the block is named after the file"
    );
    let file = read(book);
    assert_eq!(
        file.name.as_deref(),
        Some("Moby Dick.epub"),
        "the file block keeps its file's name, whose extension picks the program that opens it"
    );
    assert_eq!(file.media_type, "application/epub+zip");
    assert_eq!(file.bytes, b"PK\x03\x04\x14\x00\x00\x00book");
}
