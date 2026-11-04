use super::*;

#[test]
fn rich_text_backspace_inside_a_code_block_joins_its_lines() {
    let mut tester = EditorTester::rich(b"fn a() {\n}\n");
    tester.execute(EditorCommand::SelectAll);
    tool(&mut tester, "code_block");
    tester.set_cursor(tester.pos(9));
    tester.execute(backspace());
    assert_eq!(
        formatted(&tester),
        "[code_block=rust]fn a() {}\n",
        "a code line stays code when its line break is deleted"
    );
}
