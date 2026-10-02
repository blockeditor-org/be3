use super::*;

#[test]
fn ime_any_other_command_ends_the_composition() {
    let mut tester = EditorTester::new("say ");
    tester.set_cursor(tester.pos(4));
    tester.execute(EditorCommand::Ime(ImeCommand::SetComposingText("helo")));

    tester.execute(EditorCommand::MoveCursorLeftRight {
        mode: MoveMode::Move,
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });

    tester.expect_content("say hel|o");
    assert_eq!(tester.editor.composition(), None);
}
