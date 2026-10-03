use super::*;

#[test]
fn ime_commit_replaces_the_composition_and_ends_it() {
    let mut tester = EditorTester::new("say ");
    tester.set_cursor(tester.pos(4));
    tester.execute(EditorCommand::Ime(ImeCommand::SetComposingText("helo")));

    tester.execute(EditorCommand::Ime(ImeCommand::CommitText("hello")));

    tester.expect_content("say hello|");
    assert_eq!(tester.editor.composition(), None);
}
