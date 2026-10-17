use super::*;

#[test]
fn word_selection_in_whitespace() {
    let mut tester = EditorTester::new(b"   ");
    tester.execute(EditorCommand::Click {
        position: tester.pos(1),
        mode: DragSelectionMode::select(CursorLeftRightStop::Word),
        extend: false,
        select_syntax_node: false,
    });
    tester.expect_content(b" [ | ");
}
