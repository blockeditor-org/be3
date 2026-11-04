use super::*;

#[test]
fn visual_rows_duplicate_adds_a_caret_on_the_next_row() {
    let mut tester = EditorTester::new(b"abcdefghij");
    let grid = Grid::new(vec![0..5, 5..10]);
    tester.set_cursor(tester.pos(3));
    tester.execute(by_row(
        &grid,
        UDDirection::Down,
        VerticalMoveMode::Duplicate,
    ));
    tester.expect_content(b"abc|defgh|ij");
}
