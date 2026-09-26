use super::*;
use block_editor_beui::Waker;
use block_editor_beui::be_block::{LiveEdit, TextContent};
use text_editor_core::Document;

fn replace(document: &BlockDocument, index: usize, delete: usize, insert: &str) {
    document.edit(Vec::new(), &mut |edit| {
        edit.replace(index, delete, insert.as_bytes())
    });
    document.finish_history_group();
}

fn shared(document: &BlockDocument, content: &mut TextContent) {
    for operation in document.take_operations() {
        content.apply(&operation);
    }
    document.adopt(content);
}

#[test]
fn undo_reverts_only_what_nobody_changed_since() {
    let document = BlockDocument::new(Waker::default());
    let mut content = TextContent::from("hello world");
    document.adopt(&content);

    replace(&document, 0, 5, "HELLO");
    replace(&document, 6, 5, "WORLD");
    shared(&document, &mut content);
    assert_eq!(content.text(), "HELLO WORLD");

    content.apply(&TextOp::insert(5, " big"));
    content.apply(&TextOp::delete(10, 5));
    content.apply(&TextOp::insert(10, "earth"));
    document.adopt(&content);
    assert!(document.take_external_edit());
    assert_eq!(content.text(), "HELLO big earth");

    assert!(document.undo().is_some());
    shared(&document, &mut content);
    assert_eq!(
        content.text(),
        "HELLO big earth",
        "someone else replaced WORLD since, so its undo is skipped"
    );

    assert!(document.undo().is_some());
    shared(&document, &mut content);
    assert_eq!(content.text(), "hello big earth");

    assert!(document.redo().is_some());
    shared(&document, &mut content);
    assert_eq!(content.text(), "HELLO big earth");
    assert_eq!(document.bytes(), content.bytes());
}
