use super::*;

#[test]
fn rich_text_tab_indents_a_list_item() {
    let mut tester = EditorTester::rich(b"one\ntwo");
    tester.execute(EditorCommand::SelectAll);
    tool(&mut tester, "number");
    tester.set_cursor(tester.pos(5));
    tester.execute(EditorCommand::IndentSelection(LRDirection::Right));
    tester.execute(EditorCommand::IndentSelection(LRDirection::Right));
    assert_eq!(
        formatted(&tester),
        "[list=number]one\n[indent=2,list=number]two"
    );
    tester.execute(EditorCommand::IndentSelection(LRDirection::Left));
    assert_eq!(
        formatted(&tester),
        "[list=number]one\n[indent=1,list=number]two"
    );
    assert_eq!(tester.bytes(), b"one\ntwo", "indenting inserts no text");
}
