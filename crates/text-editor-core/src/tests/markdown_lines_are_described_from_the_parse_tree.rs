use super::*;

#[test]
fn markdown_lines_are_described_from_the_parse_tree() {
    let source = "- one\n  * [ ] two\n  - [x] three\n7. seven\n10) ten\n   cont\n\n> quote\nlazy\n\n> > a\n> - b\n\n```\n- code\n```\nplain\n";
    let mut tester = EditorTester::with_language(source.as_bytes(), TextLanguage::Markdown);
    let highlight = tester.editor.highlight();
    let look = |line: &str| {
        let start = source.find(&format!("{line}\n")).unwrap();
        let look = highlight.line_look(start);
        let marker = &source[look.marker.clone()];
        (
            marker.to_owned(),
            look.list,
            look.depth,
            look.quote,
            look.code_block,
        )
    };

    assert_eq!(
        look("- one"),
        ("- ".into(), Some(ListMarker::Bullet), 0, 0, false)
    );
    assert_eq!(
        look("  * [ ] two"),
        ("* [ ] ".into(), Some(ListMarker::Check(false)), 1, 0, false)
    );
    assert_eq!(
        look("  - [x] three"),
        ("- [x] ".into(), Some(ListMarker::Check(true)), 1, 0, false)
    );
    assert_eq!(
        look("7. seven"),
        ("7. ".into(), Some(ListMarker::Number(7)), 0, 0, false)
    );
    assert_eq!(
        look("10) ten"),
        ("10) ".into(), Some(ListMarker::Number(10)), 0, 0, false)
    );
    assert_eq!(look("   cont"), ("".into(), None, 0, 0, false));
    assert_eq!(look("> quote"), ("> ".into(), None, 0, 1, false));
    assert_eq!(look("lazy"), ("".into(), None, 0, 1, false));
    assert_eq!(look("> > a"), ("> > ".into(), None, 0, 2, false));
    assert_eq!(
        look("> - b"),
        ("> - ".into(), Some(ListMarker::Bullet), 0, 1, false)
    );
    assert_eq!(look("- code"), ("".into(), None, 0, 0, true));
    assert_eq!(look("plain"), ("".into(), None, 0, 0, false));
}
