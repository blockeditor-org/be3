use super::*;

#[test]
fn rich_text_a_heading_covers_its_line_and_enter_at_its_end_starts_a_body_line() {
    let mut tester = EditorTester::rich(b"Title\nbody");
    tester.set_cursor(tester.pos(2));
    tool(&mut tester, "heading1");
    assert_eq!(formatted(&tester), "[heading=1]Title\nbody");
    assert!(active(&tester, "heading1"));

    tester.set_cursor(tester.pos(5));
    tester.execute(EditorCommand::Newline);
    tester.execute(EditorCommand::InsertText(b"next"));
    assert_eq!(formatted(&tester), "[heading=1]Title\nnext\nbody");

    tester.set_cursor(tester.pos(2));
    tester.execute(EditorCommand::Newline);
    assert_eq!(
        formatted(&tester),
        "[heading=1]Ti\n[heading=1]tle\nnext\nbody",
        "splitting a heading keeps both halves headings"
    );

    tester.execute(EditorCommand::SelectAll);
    tool(&mut tester, "heading2");
    assert_eq!(
        formatted(&tester),
        "[heading=2]Ti\n[heading=2]tle\n[heading=2]next\n[heading=2]body"
    );
}
