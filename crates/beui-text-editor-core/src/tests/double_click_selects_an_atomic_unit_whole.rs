use super::atomic_units_are_single_cursor_units::inside_double_angle_span;
use super::*;

#[test]
fn double_click_selects_an_atomic_unit_whole() {
    let mut tester = EditorTester::new("a <<atom>> b");
    tester.editor.config.inside_atomic_unit = inside_double_angle_span;
    tester.execute(EditorCommand::Click {
        position: tester.pos(6),
        mode: DragSelectionMode::select(CursorLeftRightStop::Word),
        extend: false,
        select_syntax_node: false,
    });
    tester.expect_content(b"a [<<atom>>| b");
}
