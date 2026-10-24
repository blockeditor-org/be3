use super::*;

#[test]
fn markdown_lines_are_described_from_the_parse_tree() {
    let source = "- four\n    * spaces\n- one\n  * [ ] two\n  - [x] three\n    more three\n7. seven\n10) ten\n   cont\n\n> quote\nlazy\n\n> > a\n> - b\n>   - c\n>     more c\n\n```\n- code\n```\nplain\n";
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
            look.continues,
            look.quote,
            look.code_block,
        )
    };

    assert_eq!(
        look("    * spaces"),
        (
            "    * ".into(),
            Some(ListMarker::Bullet),
            1,
            false,
            0,
            false
        )
    );
    assert_eq!(
        look("- one"),
        ("- ".into(), Some(ListMarker::Bullet), 0, false, 0, false)
    );
    assert_eq!(
        look("  * [ ] two"),
        (
            "  * [ ] ".into(),
            Some(ListMarker::Check(false)),
            1,
            false,
            0,
            false
        )
    );
    assert_eq!(
        look("  - [x] three"),
        (
            "  - [x] ".into(),
            Some(ListMarker::Check(true)),
            1,
            false,
            0,
            false
        )
    );
    assert_eq!(
        look("7. seven"),
        (
            "7. ".into(),
            Some(ListMarker::Number(7)),
            0,
            false,
            0,
            false
        )
    );
    assert_eq!(
        look("10) ten"),
        (
            "10) ".into(),
            Some(ListMarker::Number(10)),
            0,
            false,
            0,
            false
        )
    );
    assert_eq!(look("   cont"), ("   ".into(), None, 0, true, 0, false));
    assert_eq!(look("> quote"), ("> ".into(), None, 0, false, 1, false));
    assert_eq!(look("lazy"), ("".into(), None, 0, false, 1, false));
    assert_eq!(look("> > a"), ("> > ".into(), None, 0, false, 2, false));
    assert_eq!(
        look("> - b"),
        ("> - ".into(), Some(ListMarker::Bullet), 0, false, 1, false)
    );
    assert_eq!(
        look(">   - c"),
        (
            ">   - ".into(),
            Some(ListMarker::Bullet),
            1,
            false,
            1,
            false
        )
    );
    assert_eq!(
        look("    more three"),
        ("    ".into(), None, 1, true, 0, false)
    );
    assert_eq!(
        look(">     more c"),
        (">     ".into(), None, 1, true, 1, false)
    );
    assert_eq!(look("- code"), ("".into(), None, 0, false, 0, true));
    assert_eq!(look("plain"), ("".into(), None, 0, false, 0, false));
}
