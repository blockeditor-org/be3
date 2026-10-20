use super::*;

#[test]
fn rich_text_undoing_a_join_brings_back_both_lines_blocks() {
    let mut tester = EditorTester::rich(b"A quote\nfn main() {\n}");
    tester.set_cursor(tester.pos(0));
    tool(&mut tester, "quote");
    select(&mut tester, 8..20);
    tool(&mut tester, "code_block");
    let start = "[quote]A quote\n[code_block=rust]fn main() {\n[code_block=rust]}";
    assert_eq!(formatted(&tester), start);

    tester.set_cursor(tester.pos(8));
    tester.execute(backspace());
    assert_eq!(
        formatted(&tester),
        "[quote]A quote\nfn main() {\n[code_block=rust]}",
        "backspace at the start of a code block lifts its first line out"
    );
    tester.execute(backspace());
    assert_eq!(
        formatted(&tester),
        "[quote]A quotefn main() {\n[code_block=rust]}"
    );

    tester.execute(EditorCommand::Undo);
    assert_eq!(
        formatted(&tester),
        "[quote]A quote\nfn main() {\n[code_block=rust]}",
        "undoing the join gives each line its own block again"
    );
    tester.execute(EditorCommand::Undo);
    assert_eq!(formatted(&tester), start);
}
