use super::*;

#[test]
fn word_selection_at_the_end() {
    let mut tester = EditorTester::new(b"Weekend trip");
    tester.execute(EditorCommand::Click {
        position: tester.pos(12),
        mode: DragSelectionMode::select(CursorLeftRightStop::Word),
        extend: false,
        select_syntax_node: false,
    });
    tester.expect_content(b"Weekend [trip|");
}
