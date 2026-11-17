use super::*;

/// The last line of a text is read as the same line with a newline after it: a rule, a heading
/// and a task's box look alike whether the text ends there or goes on.
#[test]
fn markdown_a_last_line_reads_as_if_a_newline_ended_it() {
    let look = |source: &str| {
        let mut tester = EditorTester::with_language(source.as_bytes(), TextLanguage::Markdown);
        let look = tester.editor.highlight().line_look(0);
        (look.rule, look.heading, look.list, source[look.marker].to_owned())
    };
    for line in ["---", "***", "# a", "- [ ] ", "- [x] ", "- [ ] a"] {
        assert_eq!(look(line), look(&format!("{line}\n")), "{line:?}");
    }
    assert!(look("---").0);
    assert_eq!(look("# a").1, 1);
    assert_eq!(
        look("- [ ] a").2,
        Some(ListMarker::Check(false)),
        "a task with text is a task"
    );
    assert_eq!(
        look("- [ ] ").2,
        Some(ListMarker::Bullet),
        "a box with nothing after it is GFM's text, as the parser reads it"
    );
}
