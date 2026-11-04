use super::*;

#[test]
fn visual_rows_select_extends_every_cursor_by_a_row() {
    let mut tester = EditorTester::new(b"abcdefghij\nklmnopqrst");
    let grid = Grid::new(vec![0..5, 5..10, 11..16, 16..21]);
    tester.set_cursor(tester.pos(1));
    tester.execute(EditorCommand::MoveCursorUpDown {
        direction: UDDirection::Down,
        mode: VerticalMoveMode::Duplicate,
        metric: CursorHorizontalPositionMetric::Byte,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });
    tester.expect_content(b"a|bcdefghij\nk|lmnopqrst");
    tester.execute(by_row(&grid, UDDirection::Down, VerticalMoveMode::Select));
    tester.expect_content(b"a[bcdef|ghij\nk[lmnop|qrst");
    tester.execute(by_row(&grid, UDDirection::Up, VerticalMoveMode::Select));
    tester.expect_content(b"a|bcdefghij\nk|lmnopqrst");
}
