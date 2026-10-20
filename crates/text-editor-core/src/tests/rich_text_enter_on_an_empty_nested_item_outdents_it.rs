use super::*;

#[test]
fn rich_text_enter_on_an_empty_nested_item_outdents_it() {
    let mut tester = EditorTester::rich(b"item\n");
    tester.set_cursor(tester.pos(0));
    tool(&mut tester, "bullet");
    tester.set_cursor(tester.pos(4));
    tester.execute(EditorCommand::Newline);
    tester.execute(EditorCommand::IndentSelection(LRDirection::Right));
    tester.execute(EditorCommand::IndentSelection(LRDirection::Right));
    assert_eq!(
        formatted(&tester),
        "[list=bullet]item\n[indent=2,list=bullet]\n"
    );

    tester.execute(EditorCommand::Newline);
    assert_eq!(
        formatted(&tester),
        "[list=bullet]item\n[indent=1,list=bullet]\n"
    );
    tester.execute(EditorCommand::Newline);
    assert_eq!(formatted(&tester), "[list=bullet]item\n[list=bullet]\n");
    tester.execute(EditorCommand::Newline);
    assert_eq!(
        formatted(&tester),
        "[list=bullet]item\n\n",
        "an empty item at the top level leaves the list"
    );
    tester.expect_content(b"item\n|\n");
}
