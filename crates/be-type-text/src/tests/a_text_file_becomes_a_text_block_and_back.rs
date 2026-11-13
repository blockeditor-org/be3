use be_block_content::BlockContent;
use be_sdk_type_guest::api::{Change, FileChange, PlainFile, guest::exports::replicas::Guest};

use super::*;

fn from_file(current: Option<Vec<u8>>, name: &str, bytes: &[u8]) -> Vec<u8> {
    from_file_by(current, name, bytes, 3)
}

fn from_file_by(current: Option<Vec<u8>>, name: &str, bytes: &[u8], client: u64) -> Vec<u8> {
    pollster::block_on(crate::block_types().apply(
        TextContent::CONTENT_TYPE,
        current,
        client,
        Change::FromFile(FileChange {
            name: name.into(),
            bytes: bytes.to_vec(),
        }),
    ))
    .expect("the file applies")
    .expect("the file is content")
}

#[test]
fn a_text_file_becomes_a_text_block_and_back() {
    let module = crate::block_types();
    let claims = |name: &str, first: &[u8]| {
        pollster::block_on(module.claims(name.into(), first.to_vec(), false, false))
    };
    assert_eq!(
        claims("notes.txt", b"hello"),
        Some(TextContent::CONTENT_TYPE)
    );
    assert_eq!(claims("photo.bin", &[0, 1, 2]), None);

    let source = b"pub fn a() {}\n";
    let content = from_file(None, "lib.rs", source);
    let decoded = TextContent::decode(&content).unwrap();
    assert_eq!(decoded.root().language, TextLanguage::Rust);
    assert_eq!(
        pollster::block_on(module.to_file(TextContent::CONTENT_TYPE, content.clone())),
        Ok(Some(PlainFile {
            bytes: source.to_vec(),
            name: None,
            extension: "rs".to_owned(),
            media_type: "text/x-rust".to_owned(),
        }))
    );

    let edited = from_file(Some(content), "lib.rs", b"pub fn a() {}\npub fn b() {}\n");
    let edited = TextContent::decode(&edited).unwrap();
    assert_eq!(TextBlock::text(&edited), "pub fn a() {}\npub fn b() {}\n");
    assert_eq!(edited.root().language, TextLanguage::Rust);

    // The host makes a new block from a file with no client of its own: client 0 is the
    // sequence's loaded buffer, which no edit may claim.
    let made = from_file_by(None, "notes.md", "# Notes\n\nÜber ✓\n".as_bytes(), 0);
    let made = TextContent::decode(&made).unwrap();
    assert_eq!(TextBlock::text(&made), "# Notes\n\nÜber ✓\n");
    assert!(
        TextBlock::body(&made).is_some_and(|body| body.is_fresh()),
        "a new block's text is the loaded buffer, with no edit authored by client 0"
    );
}
