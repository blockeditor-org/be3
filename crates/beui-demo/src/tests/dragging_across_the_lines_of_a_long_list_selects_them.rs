use super::*;

#[test]
fn dragging_across_the_lines_of_a_long_list_selects_them() {
    let mut test = alone(WIDE, Page::Rows);
    test.frame(Vec::new());
    reveal(&mut test, "rows.server");
    let log = test.rect_of("rows.server");
    let first = log.top() + 18.0;
    test.drag(
        pos2(log.left() + 14.0, first),
        pos2(log.left() + 120.0, first + 60.0),
    );
    test.frame(Vec::new());
    test.snapshot("selecting_lines_of_a_long_list");

    let region = test
        .document()
        .find_test_id("rows.server.selection")
        .expect("the log is shown");
    let selected = selected_text(&mut test, region);
    let lines: Vec<&str> = selected.lines().collect();
    assert_eq!(lines.len(), 5, "the drag covers five lines: {selected:?}");
    assert!(lines[0].starts_with("    1  listening"), "{selected:?}");
    assert!(
        lines[2].starts_with("    3  GET /style.css"),
        "{selected:?}"
    );
}
