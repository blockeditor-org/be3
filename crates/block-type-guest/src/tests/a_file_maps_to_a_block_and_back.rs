use block_type_api::{ChildChange, Plain};

use super::*;
use crate::DocumentFile;

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
struct Note {
    name: String,
    body: String,
}

impl Root for Note {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6e6f_7465);
}

impl DocumentFile for Note {
    fn to_file(content: &Document<Self>) -> Vec<u8> {
        content.root().body.clone().into_bytes()
    }

    fn from_file(
        current: Option<&Document<Self>>,
        name: &str,
        bytes: Vec<u8>,
        _: u64,
    ) -> Option<Document<Self>> {
        let mut note = current.cloned().unwrap_or_else(|| {
            Document::new(&Note {
                name: name.to_owned(),
                body: String::new(),
            })
        });
        note.apply(&Edit::from(
            Note::BODY.set(ObjectId::ROOT, &String::from_utf8(bytes).ok()?),
        ));
        Some(note)
    }

    fn claims(name: &str, _: &[u8]) -> bool {
        name.ends_with(".note")
    }
}

type NoteContent = Document<Note>;

crate::be_block_types![
    NoteContent { undo, file },
    FileContent { file },
    FolderContent { directory },
];

fn ask(module: &mut Module, request: Request) -> Reply {
    match module.handle(Event::Request { id: 1, request }).as_slice() {
        [Command::Reply { id: 1, reply }] => reply.clone(),
        other => panic!("expected one reply, got {other:?}"),
    }
}

fn claimed_as(module: &mut Module, name: &str, first: &[u8], fallback: bool) -> Option<Uuid> {
    match ask(
        module,
        Request::Claims {
            name: name.into(),
            first: first.to_vec(),
            directory: false,
            fallback,
        },
    ) {
        Reply::Claimed(claimed) => claimed,
        other => panic!("expected a claim, got {other:?}"),
    }
}

fn claimed(module: &mut Module, name: &str, first: &[u8]) -> Option<Uuid> {
    claimed_as(module, name, first, false).or_else(|| claimed_as(module, name, first, true))
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
    assert_eq!(plain(NoteContent::CONTENT_TYPE), Some(Plain::File));

    let note = claimed(&mut module, "plan.note", b"one").unwrap();
    assert_eq!(note, NoteContent::CONTENT_TYPE);
    let content = from_file(&mut module, note, None, "plan.note", b"one");
    assert_eq!(
        NoteContent::decode(&content).unwrap().root().name,
        "plan.note"
    );
    assert_eq!(to_file(&mut module, note, content.clone()).unwrap(), b"one");
    let edited = from_file(&mut module, note, Some(content), "renamed.note", b"two");
    let edited = NoteContent::decode(&edited).unwrap();
    assert_eq!(edited.root().name, "plan.note");
    assert_eq!(edited.root().body, "two");

    assert_eq!(
        claimed_as(&mut module, "frame.paint", &[0, 159, 255], false),
        None
    );
    assert_eq!(
        claimed_as(&mut module, "plan.note", b"one", true),
        Some(FileContent::CONTENT_TYPE)
    );
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
                fallback: false,
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
