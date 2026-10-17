use super::*;

#[test]
fn word_selection_respects_atomic_units() {
    let mut tester = EditorTester::new(b"a <<atom>> b");
    tester.editor.config.inside_atomic_unit = |_, index| (3..10).contains(&index);
    tester.execute(EditorCommand::Click {
        position: tester.pos(6),
        mode: DragSelectionMode::select(CursorLeftRightStop::Word),
        extend: false,
        select_syntax_node: false,
    });
    tester.expect_content(b"a [<<atom>>| b");
}
