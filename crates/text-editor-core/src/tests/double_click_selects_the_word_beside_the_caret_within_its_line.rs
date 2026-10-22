use super::*;

#[test]
fn double_click_selects_the_word_beside_the_caret_within_its_line() {
    let mut tester = EditorTester::new(b"abc def\n\nghi");
    let mut word_click = |index: usize, expected: &[u8]| {
        tester.execute(EditorCommand::Click {
            position: tester.pos(index),
            mode: DragSelectionMode::select(CursorLeftRightStop::Word),
            extend: false,
            select_syntax_node: false,
        });
        tester.expect_content(expected);
    };
    word_click(3, b"[abc| def\n\nghi");
    word_click(4, b"abc [def|\n\nghi");
    word_click(7, b"abc [def|\n\nghi");
    word_click(8, b"abc def\n|\nghi");
    word_click(12, b"abc def\n\n[ghi|");
}
