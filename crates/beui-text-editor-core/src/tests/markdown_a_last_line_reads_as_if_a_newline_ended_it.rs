use super::*;

/// The last line of a text is read as the same line with a newline after it: a rule, a heading
/// and an empty task look alike whether the text ends there or goes on.
#[test]
fn markdown_a_last_line_reads_as_if_a_newline_ended_it() {
    let look = |source: &str| {
        let mut tester = EditorTester::with_language(source.as_bytes(), TextLanguage::Markdown);
        let look = tester.editor.highlight().line_look(0);
        (look.rule, look.heading, look.list, source[look.marker].to_owned())
    };
    for line in ["---", "***", "# a", "- [ ] ", "- [x] ", "- [ ]a"] {
        assert_eq!(look(line), look(&format!("{line}\n")), "{line:?}");
    }
    assert!(look("---").0);
    assert_eq!(look("# a").1, 1);
    assert_eq!(
        look("- [ ] "),
        (false, 0, Some(ListMarker::Check(false)), "- [ ] ".into())
    );
    assert_eq!(
        look("- [x] \n").2,
        Some(ListMarker::Check(true)),
        "an empty task is a task"
    );
    assert_eq!(
        look("- [ ]\n").2,
        Some(ListMarker::Bullet),
        "a task's box is followed by a space"
    );
    assert_eq!(
        look("- [ ] \nmore\n").2,
        Some(ListMarker::Bullet),
        "a paragraph that goes on past the box is GFM's text"
    );
}
