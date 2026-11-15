use super::*;

#[test]
fn triple_click_at_the_end_selects_the_last_line() {
    for (text, expected) in [
        ("Weekend trip", "[Weekend trip|"),
        ("first\nlast", "first\n[last|"),
        ("first\n", "first\n|"),
        ("", "|"),
    ] {
        let mut tester = EditorTester::new(text);
        tester.execute(EditorCommand::Click {
            position: tester.pos(text.len()),
            mode: DragSelectionMode::select(CursorLeftRightStop::Line),
            extend: false,
            select_syntax_node: false,
        });
        tester.expect_content(expected.as_bytes());
    }
}
