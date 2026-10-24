use crate::plain::{self, Form};
use crate::{
    BlockContent, FileContent, FolderContent, ImageContent, TextBlock, TextContent, TextLanguage,
};

#[test]
fn a_file_maps_to_a_block_and_back() {
    let organization = uuid::Uuid::nil();
    let text = plain::new_block("lib.rs", b"pub fn a() {}\n".to_vec(), organization);
    assert_eq!(text.block_type, TextContent::CONTENT_TYPE);
    let decoded = TextContent::decode(&text.content).unwrap();
    assert_eq!(decoded.root().language, TextLanguage::Rust);
    assert_eq!(
        plain::file_bytes(text.block_type, &text.content).unwrap(),
        b"pub fn a() {}\n"
    );

    let binary = plain::new_block("frame.paint", vec![0, 159, 255], organization);
    assert_eq!(binary.block_type, FileContent::CONTENT_TYPE);
    assert_eq!(
        plain::file_bytes(binary.block_type, &binary.content).unwrap(),
        [0, 159, 255]
    );
    let image = plain::new_block("logo.png", vec![0x89, b'P', 0], organization);
    assert_eq!(image.block_type, ImageContent::CONTENT_TYPE);

    let edited = plain::update_block(
        text.block_type,
        Some(&text.content),
        "lib.rs",
        b"pub fn a() {}\npub fn b() {}\n".to_vec(),
        3,
        organization,
    )
    .unwrap();
    let edited = TextContent::decode(&edited.content).unwrap();
    assert_eq!(TextBlock::text(&edited), "pub fn a() {}\npub fn b() {}\n");
    assert_eq!(edited.root().language, TextLanguage::Rust);

    assert_eq!(plain::form(FolderContent::CONTENT_TYPE), Form::Directory);
    assert_eq!(plain::form(crate::CanvasContent::CONTENT_TYPE), Form::None);
    assert_eq!(plain::export_name("Trip notes", "md"), "Trip notes.md");
    assert_eq!(plain::export_name("notes.md", "md"), "notes.md");
}
