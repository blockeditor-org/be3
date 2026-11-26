use super::*;

/// A rule on the last line is a rule whether or not a newline follows it, so typing Enter after
/// it and Backspace again never turns it into a paragraph.
#[test]
fn markdown_a_rule_at_the_end_stays_a_rule_through_enter_and_backspace() {
    let mut tester = EditorTester::with_language(b"Above\n\n---", TextLanguage::Markdown);
    let rule = |tester: &mut EditorTester| tester.editor.highlight().line_look(7).rule;
    assert!(rule(&mut tester));

    tester.set_cursor(tester.pos(usize::MAX));
    tester.execute(EditorCommand::Newline);
    tester.expect_content(b"Above\n\n---\n|");
    assert!(rule(&mut tester), "after Enter");

    tester.execute(EditorCommand::Delete {
        direction: LRDirection::Left,
        stop: CursorLeftRightStop::UnicodeGraphemeCluster,
    });
    tester.expect_content(b"Above\n\n---|");
    assert!(rule(&mut tester), "after Backspace");
}
