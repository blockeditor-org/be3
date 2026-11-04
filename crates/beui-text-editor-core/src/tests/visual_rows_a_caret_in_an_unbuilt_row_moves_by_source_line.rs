use super::*;

#[test]
fn visual_rows_a_caret_in_an_unbuilt_row_moves_by_source_line() {
    let mut tester = EditorTester::new(b"abcdefghij\nxyz");
    let grid = Grid {
        rows: vec![0..5, 5..10, 11..14],
        built: 2..3,
    };
    tester.set_cursor(tester.pos(13));
    tester.execute(by_row(&grid, UDDirection::Up, VerticalMoveMode::Move));
    tester.expect_content(b"ab|cdefghij\nxyz");
}
