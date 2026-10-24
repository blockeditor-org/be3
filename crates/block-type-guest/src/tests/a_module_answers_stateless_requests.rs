use be_block::{TextBlock, TextContent};

use super::*;

crate::be_block_types![ChecklistContent { undo }, TextContent { undo, file }];

#[test]
fn a_module_answers_stateless_requests() {
    let mut module = block_types();
    let Command::Hello { types } = module.hello() else {
        panic!("a module greets with its types");
    };
    assert_eq!(types.len(), 2);
    assert!(types[0].undo && !types[0].file);
    assert!(types[1].undo && types[1].file);
    let mut ask =
        |request: Request| match module.handle(Event::Request { id: 1, request }).as_slice() {
            [Command::Reply { id: 1, reply }] => reply.clone(),
            other => panic!("expected one reply, got {other:?}"),
        };
    let Reply::Blank(blank) = ask(Request::Blank {
        content_type: ChecklistContent::CONTENT_TYPE,
    }) else {
        panic!("a blank checklist");
    };
    assert_eq!(blank, ChecklistContent::default().encode());
    let mut ours = ChecklistContent::default();
    ours.apply(&add("ours"));
    let mut theirs = ChecklistContent::default();
    theirs.apply(&add("theirs"));
    let Reply::Merged { bytes, conflicts } = ask(Request::Merge3 {
        content_type: ChecklistContent::CONTENT_TYPE,
        base: None,
        ours: Some(ours.encode()),
        theirs: Some(theirs.encode()),
    }) else {
        panic!("a merge");
    };
    assert_eq!(conflicts, 0);
    assert_eq!(texts(&ChecklistContent::decode(&bytes).unwrap()).len(), 2);
    let Reply::Described(description) = ask(Request::Describe {
        content_type: ChecklistContent::CONTENT_TYPE,
        bytes: ours.encode(),
        organization: ORGANIZATION,
    }) else {
        panic!("a description");
    };
    assert_eq!(description.derived, be_block::derived_metadata(&ours));
    let text = TextBlock::of("hello");
    assert_eq!(
        ask(Request::ToFile {
            content_type: TextContent::CONTENT_TYPE,
            bytes: text.encode(),
        }),
        Reply::File(Some(b"hello".to_vec()))
    );
    assert_eq!(
        ask(Request::ToFile {
            content_type: ChecklistContent::CONTENT_TYPE,
            bytes: ours.encode(),
        }),
        Reply::File(None)
    );
    assert_eq!(
        ask(Request::Claims {
            name: "notes.txt".into(),
            first: b"hello".to_vec(),
        }),
        Reply::Claimed(Some(TextContent::CONTENT_TYPE))
    );
    assert_eq!(
        ask(Request::Claims {
            name: "photo.bin".into(),
            first: vec![0, 1, 2],
        }),
        Reply::Claimed(None)
    );
    assert_eq!(
        ask(Request::Blank {
            content_type: Uuid::nil(),
        }),
        Reply::Unknown
    );
    assert_eq!(ask(Request::Content { replica: 9 }), Reply::Unknown);
}
