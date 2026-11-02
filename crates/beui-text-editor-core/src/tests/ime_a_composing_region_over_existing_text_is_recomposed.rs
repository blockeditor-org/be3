use super::*;

#[test]
fn ime_a_composing_region_over_existing_text_is_recomposed() {
    let mut tester = EditorTester::new("say helo there");
    tester.set_cursor(tester.pos(14));

    tester.execute(EditorCommand::Ime(ImeCommand::SetComposingRegion(4..8)));
    assert_eq!(tester.editor.composition(), Some(4..8));
    tester.expect_content("say helo there|");

    tester.execute(EditorCommand::Ime(ImeCommand::CommitText("hello")));
    tester.expect_content("say hello| there");
    assert_eq!(tester.editor.composition(), None);
}
