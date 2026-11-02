use super::*;

#[test]
fn ime_composing_text_is_written_into_the_document_and_replaced_as_it_changes() {
    let mut tester = EditorTester::new("say ");
    tester.set_cursor(tester.pos(4));

    tester.execute(EditorCommand::Ime(ImeCommand::SetComposingText("he")));
    tester.expect_content("say he|");
    assert_eq!(tester.editor.composition(), Some(4..6));

    tester.execute(EditorCommand::Ime(ImeCommand::SetComposingText("helo")));
    tester.expect_content("say helo|");
    assert_eq!(tester.editor.composition(), Some(4..8));
}
