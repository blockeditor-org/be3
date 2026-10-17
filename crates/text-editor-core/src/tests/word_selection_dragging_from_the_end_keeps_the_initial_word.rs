use super::*;

#[test]
fn word_selection_dragging_from_the_end_keeps_the_initial_word() {
    let mut tester = EditorTester::new(b"Weekend trip");
    tester.execute(EditorCommand::Click {
        position: Position::END,
        mode: DragSelectionMode::select(CursorLeftRightStop::Word),
        extend: false,
        select_syntax_node: false,
    });
    tester.execute(EditorCommand::Drag(tester.pos(7)));
    tester.expect_content(b"|Weekend trip]");
    tester.execute(EditorCommand::Drag(Position::END));
    tester.expect_content(b"Weekend [trip|");
}
