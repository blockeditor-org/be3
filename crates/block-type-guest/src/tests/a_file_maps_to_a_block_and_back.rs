use be_block::{FileContent, TextBlock, TextContent, TextLanguage};
use block_type_api::{ChildChange, Plain};

use super::*;

crate::be_block_types![
    TextContent { undo, file },
    FileContent { file },
    FolderContent { directory },
];

fn ask(module: &mut Module, request: Request) -> Reply {
    match module.handle(Event::Request { id: 1, request }).as_slice() {
        [Command::Reply { id: 1, reply }] => reply.clone(),
        other => panic!("expected one reply, got {other:?}"),
    }
}

fn claimed(module: &mut Module, name: &str, first: &[u8]) -> Option<Uuid> {
    match ask(
        module,
        Request::Claims {
            name: name.into(),
            first: first.to_vec(),
            directory: false,
        },
    ) {
        Reply::Claimed(claimed) => claimed,
        other => panic!("expected a claim, got {other:?}"),
    }
}

fn from_file(
    module: &mut Module,
    content_type: Uuid,
    current: Option<Vec<u8>>,
    name: &str,
    bytes: &[u8],
) -> Vec<u8> {
    match ask(
        module,
        Request::Apply {
            content_type,
            bytes: current,
            client: 3,
            change: Change::FromFile {
                name: name.into(),
                bytes: bytes.to_vec(),
            },
        },
    ) {
        Reply::Content(bytes) => bytes,
        other => panic!("expected content, got {other:?}"),
    }
}

fn to_file(module: &mut Module, content_type: Uuid, bytes: Vec<u8>) -> Option<Vec<u8>> {
    match ask(
        module,
        Request::ToFile {
            content_type,
            bytes,
        },
    ) {
        Reply::File(file) => file,
        other => panic!("expected a file, got {other:?}"),
    }
}

#[test]
fn a_file_maps_to_a_block_and_back() {
    let mut outbound = Vec::new();
    <BlockTypes as crate::abi::Guest>::start(crate::abi::Waker::new(|| {}), &mut outbound).unwrap();
    let mut module = block_types();
    let Command::Hello { types } = module.hello() else {
        panic!("a module greets with its types");
    };
    let plain = |content_type: Uuid| {
        types
            .iter()
            .find(|info| info.content_type == content_type)
            .map(|info| info.plain)
    };
    assert_eq!(plain(FolderContent::CONTENT_TYPE), Some(Plain::Directory));
    assert_eq!(plain(TextContent::CONTENT_TYPE), Some(Plain::File));

    let source = b"pub fn a() {}\n";
    let text = claimed(&mut module, "lib.rs", source).unwrap();
    assert_eq!(text, TextContent::CONTENT_TYPE);
    let content = from_file(&mut module, text, None, "lib.rs", source);
    let decoded = TextContent::decode(&content).unwrap();
    assert_eq!(decoded.root().language, TextLanguage::Rust);
    assert_eq!(to_file(&mut module, text, content.clone()).unwrap(), source);

    let edited = from_file(
        &mut module,
        text,
        Some(content),
        "lib.rs",
        b"pub fn a() {}\npub fn b() {}\n",
    );
    let edited = TextContent::decode(&edited).unwrap();
    assert_eq!(TextBlock::text(&edited), "pub fn a() {}\npub fn b() {}\n");
    assert_eq!(edited.root().language, TextLanguage::Rust);

    let binary = claimed(&mut module, "frame.paint", &[0, 159, 255]).unwrap();
    assert_eq!(binary, FileContent::CONTENT_TYPE);
    let content = from_file(&mut module, binary, None, "frame.paint", &[0, 159, 255]);
    assert_eq!(
        to_file(&mut module, binary, content).unwrap(),
        [0, 159, 255]
    );

    assert_eq!(
        ask(
            &mut module,
            Request::Claims {
                name: "src".into(),
                first: Vec::new(),
                directory: true,
            }
        ),
        Reply::Claimed(Some(FolderContent::CONTENT_TYPE))
    );
    let child = Uuid::from_u128(0xc41d);
    let Reply::Content(folder) = ask(
        &mut module,
        Request::Apply {
            content_type: FolderContent::CONTENT_TYPE,
            bytes: None,
            client: 3,
            change: Change::Child(ChildChange::Add(child)),
        },
    ) else {
        panic!("a folder takes a child");
    };
    assert_eq!(
        FolderContent::decode(&folder).unwrap().references(),
        vec![child]
    );
    assert_eq!(
        to_file(&mut module, FolderContent::CONTENT_TYPE, folder),
        None
    );
}
