use super::*;

#[test]
fn visual_rows_a_row_inside_a_collapsed_section_falls_back_to_source_lines() {
    let content = "# A\nbody a1\nbody a2\n# B\nbody b\n";
    let mut tester = EditorTester::with_language(content.as_bytes(), TextLanguage::Markdown);
    let b_start = content.find("# B").unwrap();
    tester.set_cursor(tester.pos(1));
    tester.execute(EditorCommand::Collapse);

    let rows = [0, 4, 12, b_start, b_start + 4, content.len()]
        .windows(2)
        .map(|pair| pair[0]..pair[1] - 1)
        .collect();
    let grid = Grid::new(rows);
    tester.execute(by_row(&grid, UDDirection::Down, VerticalMoveMode::Move));

    let cursor = tester.editor.cursor_positions()[0];
    let focus = tester.editor.position_index(cursor.pos.focus).unwrap();
    assert_eq!(focus, b_start + 1);
}
