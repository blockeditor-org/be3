use super::*;

#[test]
fn word_selection_before_a_space() {
    let mut tester = EditorTester::new(b"Weekend trip");
    tester.execute(EditorCommand::Click {
        position: tester.pos(7),
        mode: DragSelectionMode::select(CursorLeftRightStop::Word),
        extend: false,
        select_syntax_node: false,
    });
    tester.expect_content(b"[Weekend| trip");
}
