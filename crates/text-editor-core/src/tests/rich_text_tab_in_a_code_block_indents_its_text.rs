use super::*;

#[test]
fn rich_text_tab_in_a_code_block_indents_its_text() {
    let mut tester = EditorTester::rich(b"fn main() {\nbody();\n}");
    select(&mut tester, 0..20);
    tool(&mut tester, "code_block");
    tester.set_cursor(tester.pos(12));
    tester.execute(EditorCommand::IndentSelection(LRDirection::Right));
    assert_eq!(
        formatted(&tester),
        "[code_block=rust]fn main() {\n[code_block=rust]  body();\n[code_block=rust]}"
    );
    tester.expect_content(b"fn main() {\n  |body();\n}");
}
