use super::*;

#[test]
fn rich_text_a_style_toggled_at_the_caret_carries_across_enter() {
    let mut tester = EditorTester::rich(b"ab");
    tester.set_cursor(tester.pos(2));
    tool(&mut tester, "bold");
    tester.execute(EditorCommand::Newline);
    assert!(active(&tester, "bold"), "the new line still types bold");
    tester.execute(EditorCommand::InsertText(b"c"));
    assert_eq!(formatted(&tester), "ab{bold}\nc");
}
