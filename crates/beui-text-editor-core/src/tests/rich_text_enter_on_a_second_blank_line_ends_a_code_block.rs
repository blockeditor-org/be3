use super::*;

#[test]
fn rich_text_enter_on_a_second_blank_line_ends_a_code_block() {
    let mut tester = EditorTester::rich(b"let a = 1;");
    tester.set_cursor(tester.pos(0));
    tool(&mut tester, "code_block");
    tester.set_cursor(tester.pos(10));
    tester.execute(EditorCommand::Newline);
    tester.execute(EditorCommand::Newline);
    assert_eq!(
        formatted(&tester),
        "[code_block=rust]let a = 1;\n[code_block=rust]\n"
    );
    assert!(
        active(&tester, "code_block"),
        "one blank line stays in the code"
    );
    tester.execute(EditorCommand::Newline);
    assert_eq!(formatted(&tester), "[code_block=rust]let a = 1;\n");
    assert!(!active(&tester, "code_block"));
    tester.expect_content(b"let a = 1;\n|");
    tester.execute(EditorCommand::InsertText(b"text"));
    assert_eq!(formatted(&tester), "[code_block=rust]let a = 1;\ntext");

    tester.execute(EditorCommand::Undo);
    tester.execute(EditorCommand::Undo);
    assert_eq!(
        formatted(&tester),
        "[code_block=rust]let a = 1;\n[code_block=rust]\n"
    );
    assert!(active(&tester, "code_block"));

    let mut tester = EditorTester::rich(b"a\n\n\nb");
    select(&mut tester, 0..4);
    tool(&mut tester, "code_block");
    tester.set_cursor(tester.pos(3));
    tester.execute(EditorCommand::Newline);
    assert_eq!(
        formatted(&tester),
        "[code_block=rust]a\n[code_block=rust]\n[code_block=rust]\n[code_block=rust]\n[code_block=rust]b",
        "blank lines inside a code block stay code"
    );
}
