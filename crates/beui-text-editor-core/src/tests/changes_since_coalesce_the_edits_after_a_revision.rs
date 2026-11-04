use super::*;

#[test]
fn changes_since_coalesce_the_edits_after_a_revision() {
    let mut tester = EditorTester::with_language(b"hello world", TextLanguage::Markdown);
    let start = tester.editor.document().revision();

    tester.set_cursor(tester.pos(5));
    tester.execute(EditorCommand::InsertText(b","));
    tester.set_cursor(tester.pos(0));
    tester.execute(EditorCommand::InsertText(b"Oh "));
    let document = tester.editor.document();

    assert_eq!(
        document.changes_since(start),
        Some(TextChange {
            start: 0,
            old_end: 5,
            new_end: 9,
        })
    );
    assert_eq!(
        document.changes_since(document.revision()),
        Some(TextChange::NONE)
    );
    document.set_language(TextLanguage::Rust);
    assert_eq!(document.changes_since(start), None);
}
