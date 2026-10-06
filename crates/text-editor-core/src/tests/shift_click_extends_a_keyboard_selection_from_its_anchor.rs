use super::*;

#[test]
fn shift_click_extends_a_keyboard_selection_from_its_anchor() {
    let mut tester = EditorTester::new(b"abcdefghi");
    tester.set_cursor(tester.pos(3));
    for _ in 0..2 {
        tester.execute(EditorCommand::MoveCursorLeftRight {
            direction: LRDirection::Right,
            mode: MoveMode::Select,
            stop: CursorLeftRightStop::UnicodeGraphemeCluster,
        });
    }
    tester.expect_content(b"abc[de|fghi");
    tester.execute(EditorCommand::Click {
        position: tester.pos(8),
        mode: DragSelectionMode::default(),
        extend: true,
        select_syntax_node: false,
    });
    tester.expect_content(b"abc[defgh|i");
}
