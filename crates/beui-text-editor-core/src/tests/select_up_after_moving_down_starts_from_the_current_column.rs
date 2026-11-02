use super::*;

#[test]
fn select_up_after_moving_down_starts_from_the_current_column() {
    let mut tester = EditorTester::new(b"abcdef\ng");
    tester.set_cursor(tester.pos(3));
    tester.execute(EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Down,
        mode: VerticalMoveMode::Move,
        metric: CursorHorizontalPositionMetric::Byte,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });
    tester.expect_content(b"abcdef\ng|");
    tester.execute(EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Up,
        mode: VerticalMoveMode::Select,
        metric: CursorHorizontalPositionMetric::Byte,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });
    tester.expect_content(b"a|bcdef\ng]");
}
