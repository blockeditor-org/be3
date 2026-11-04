use super::*;

#[test]
fn dragging_back_after_selecting_past_the_edge_follows_the_pointer() {
    let mut tester = EditorTester::new(b"one\ntwo\nthree");

    tester.execute(EditorCommand::Click {
        position: tester.pos(1),
        mode: DragSelectionMode::default(),
        extend: false,
        select_syntax_node: false,
    });
    tester.execute(EditorCommand::Drag(tester.pos(5)));
    tester.expect_content(b"o[ne\nt|wo\nthree");
    tester.execute(EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Down,
        mode: VerticalMoveMode::Select,
        metric: CursorHorizontalPositionMetric::Byte,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });
    tester.expect_content(b"o[ne\ntwo\nt|hree");
    tester.execute(EditorCommand::Drag(tester.pos(2)));
    tester.expect_content(b"o[n|e\ntwo\nthree");
}
