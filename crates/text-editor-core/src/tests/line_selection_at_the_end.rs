use super::*;

#[test]
fn line_selection_at_the_end() {
    for (text, expected) in [
        ("Weekend trip", "[Weekend trip|"),
        ("first\nlast", "first\n[last|"),
        ("first\n", "first\n|"),
        ("", "|"),
    ] {
        let mut tester = EditorTester::new(text.as_bytes());
        tester.execute(EditorCommand::Click {
            position: Position::END,
            mode: DragSelectionMode::select(CursorLeftRightStop::Line),
            extend: false,
            select_syntax_node: false,
        });
        tester.expect_content(expected.as_bytes());
    }
}
