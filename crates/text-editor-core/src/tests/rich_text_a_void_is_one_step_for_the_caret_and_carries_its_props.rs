use super::*;

#[test]
fn rich_text_a_void_is_one_step_for_the_caret_and_carries_its_props() {
    let mut tester = EditorTester::rich(b"hi !");
    tester.set_cursor(tester.pos(3));
    tester.execute(EditorCommand::InsertVoid(MarkSet::new([(
        Name::new("mention"),
        Value::text("ada"),
    )])));
    tester.execute(EditorCommand::InsertText(b" "));
    assert_eq!(
        formatted(&tester),
        "hi {mention=ada}\u{ef}\u{bf}\u{bc}{} !",
        "a void carries its props and text typed after it does not"
    );
    tester.execute(EditorCommand::MoveCursorLeftRight {
        mode: MoveMode::Move,
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });
    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });
    assert_eq!(
        tester.bytes(),
        b"hi  !",
        "one backspace removes the whole void"
    );
}
