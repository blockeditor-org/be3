use super::*;

#[test]
fn visual_rows_up_and_down_follow_wrapped_rows() {
    let mut tester = EditorTester::new(b"abcdefghij\nxy");
    let grid = Grid::new(vec![0..5, 5..10, 11..13]);
    tester.set_cursor(tester.pos(2));
    tester.execute(by_row(&grid, UDDirection::Down, VerticalMoveMode::Move));
    tester.expect_content(b"abcdefg|hij\nxy");
    tester.execute(by_row(&grid, UDDirection::Down, VerticalMoveMode::Move));
    tester.expect_content(b"abcdefghij\nxy|");
    tester.execute(by_row(&grid, UDDirection::Down, VerticalMoveMode::Move));
    tester.expect_content(b"abcdefghij\nxy|");
    tester.execute(by_row(&grid, UDDirection::Up, VerticalMoveMode::Move));
    tester.expect_content(b"abcdefg|hij\nxy");
    tester.execute(by_row(&grid, UDDirection::Up, VerticalMoveMode::Move));
    tester.expect_content(b"ab|cdefghij\nxy");
    tester.execute(by_row(&grid, UDDirection::Up, VerticalMoveMode::Move));
    tester.expect_content(b"|abcdefghij\nxy");
}
