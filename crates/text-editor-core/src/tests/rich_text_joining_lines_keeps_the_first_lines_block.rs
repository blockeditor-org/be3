use super::*;

#[test]
fn rich_text_joining_lines_keeps_the_first_lines_block() {
    let mut tester = EditorTester::rich(b"quoted\nafter");
    tester.set_cursor(tester.pos(2));
    tool(&mut tester, "quote");
    tester.set_cursor(tester.pos(6));
    tester.execute(EditorCommand::Newline);
    tester.execute(EditorCommand::Newline);
    assert_eq!(
        formatted(&tester),
        "[quote]quoted\n\nafter",
        "the second Enter lifts the new line out of the quote"
    );
    tester.execute(backspace());
    assert_eq!(
        formatted(&tester),
        "[quote]quoted\nafter",
        "joining the empty line into the quote keeps the quote"
    );
    tester.expect_content(b"quoted|\nafter");
}
