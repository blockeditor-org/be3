use super::*;

#[test]
fn markdown_list_newline() {
    let mut tester = EditorTester::with_language(b"- first", TextLanguage::Markdown);
    tester.set_cursor(tester.pos(usize::MAX));
    tester.execute(EditorCommand::Newline);
    tester.expect_content(b"- first\n- |");

    tester.execute(EditorCommand::InsertText(b"second"));
    tester.execute(EditorCommand::Newline);
    tester.expect_content(b"- first\n- second\n- |");
    tester.execute(EditorCommand::Newline);
    tester.expect_content(b"- first\n- second\n|");

    tester.execute(EditorCommand::InsertText(b"9. ninth"));
    tester.execute(EditorCommand::Newline);
    tester.expect_content(b"- first\n- second\n9. ninth\n10. |");
}
