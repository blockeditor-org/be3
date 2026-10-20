use super::*;

#[test]
fn rich_text_undo_and_redo_bring_formatting_back() {
    let mut tester = EditorTester::rich(b"abcdef");
    select(&mut tester, 0..4);
    tool(&mut tester, "strikethrough");
    select(&mut tester, 2..6);
    tool(&mut tester, "code");
    assert_eq!(
        formatted(&tester),
        "{strikethrough}ab{code,strikethrough}cd{code}ef"
    );

    tester.execute(EditorCommand::Undo);
    assert_eq!(formatted(&tester), "{strikethrough}abcd{}ef");
    tester.execute(EditorCommand::Undo);
    assert_eq!(formatted(&tester), "abcdef");
    tester.execute(EditorCommand::Redo);
    assert_eq!(formatted(&tester), "{strikethrough}abcd{}ef");
    tester.execute(EditorCommand::Redo);
    assert_eq!(
        formatted(&tester),
        "{strikethrough}ab{code,strikethrough}cd{code}ef"
    );

    select(&mut tester, 1..3);
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });
    assert_eq!(
        formatted(&tester),
        "{strikethrough}a{code,strikethrough}d{code}ef"
    );
    tester.execute(EditorCommand::Undo);
    assert_eq!(
        formatted(&tester),
        "{strikethrough}ab{code,strikethrough}cd{code}ef",
        "deleted text comes back with its formatting"
    );
}
