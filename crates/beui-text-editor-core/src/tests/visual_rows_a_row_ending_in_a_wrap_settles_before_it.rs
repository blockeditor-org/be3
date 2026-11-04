use super::*;

#[test]
fn visual_rows_a_row_ending_in_a_wrap_settles_before_it() {
    let mut tester = EditorTester::new(b"ab cdefgh");
    let grid = Grid::new(vec![0..3, 3..9]);
    tester.set_cursor(tester.pos(8));
    tester.execute(by_row(&grid, UDDirection::Up, VerticalMoveMode::Move));
    tester.expect_content(b"ab| cdefgh");
    tester.execute(by_row(&grid, UDDirection::Down, VerticalMoveMode::Move));
    tester.expect_content(b"ab cdefg|h");
}
