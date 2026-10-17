use super::*;

#[test]
fn word_selection_after_a_space() {
    let mut tester = EditorTester::new(b"Weekend trip");
    tester.execute(EditorCommand::Click {
        position: tester.pos(8),
        mode: DragSelectionMode::select(CursorLeftRightStop::Word),
        extend: false,
        select_syntax_node: false,
    });
    tester.expect_content(b"Weekend [trip|");
}
