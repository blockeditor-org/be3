use be_block::BlockContent;
use block_type_guest::api::{Change, Command, Event, Reply, Request};

use super::*;

fn ask(request: Request) -> Reply {
    match crate::block_types()
        .handle(Event::Request { id: 1, request })
        .as_slice()
    {
        [Command::Reply { id: 1, reply }] => reply.clone(),
        other => panic!("expected one reply, got {other:?}"),
    }
}

fn from_file(current: Option<Vec<u8>>, name: &str, bytes: &[u8]) -> Vec<u8> {
    match ask(Request::Apply {
        content_type: TextContent::CONTENT_TYPE,
        bytes: current,
        client: 3,
        change: Change::FromFile {
            name: name.into(),
            bytes: bytes.to_vec(),
        },
    }) {
        Reply::Content(bytes) => bytes,
        other => panic!("expected content, got {other:?}"),
    }
}

#[test]
fn a_text_file_becomes_a_text_block_and_back() {
    let claims = |name: &str, first: &[u8]| {
        ask(Request::Claims {
            name: name.into(),
            first: first.to_vec(),
            directory: false,
            fallback: false,
        })
    };
    assert_eq!(
        claims("notes.txt", b"hello"),
        Reply::Claimed(Some(TextContent::CONTENT_TYPE))
    );
    assert_eq!(claims("photo.bin", &[0, 1, 2]), Reply::Claimed(None));

    let source = b"pub fn a() {}\n";
    let content = from_file(None, "lib.rs", source);
    let decoded = TextContent::decode(&content).unwrap();
    assert_eq!(decoded.root().language, TextLanguage::Rust);
    assert_eq!(
        ask(Request::ToFile {
            content_type: TextContent::CONTENT_TYPE,
            bytes: content.clone(),
        }),
        Reply::File(Some(source.to_vec()))
    );

    let edited = from_file(Some(content), "lib.rs", b"pub fn a() {}\npub fn b() {}\n");
    let edited = TextContent::decode(&edited).unwrap();
    assert_eq!(TextBlock::text(&edited), "pub fn a() {}\npub fn b() {}\n");
    assert_eq!(edited.root().language, TextLanguage::Rust);
}
