use super::*;

#[test]
fn styles_read_before_an_edit_are_redone_around_it() {
    let mut text = String::new();
    for line in 0..3000 {
        text.push_str(&format!("line {line} with *some* words\n"));
    }
    let mut tester = EditorTester::with_language(text.as_bytes(), TextLanguage::Markdown);
    let before = tester.editor.highlight();
    for at in (0..text.len()).step_by(4096) {
        before.style_at(at);
    }

    let middle = text.find("line 1500 ").unwrap();
    tester.set_cursor(tester.pos(middle));
    tester.execute(EditorCommand::InsertText(b"# "));
    let edited = tester.bytes();
    let after = tester.editor.highlight();
    let fresh = EditorTester::with_language(&edited, TextLanguage::Markdown)
        .editor
        .highlight();

    assert!(matches!(
        after.style_at(middle + 2).size,
        SynHlTextSize::Heading(1)
    ));
    for at in (0..edited.len()).step_by(97) {
        assert_eq!(
            after.style_at(at),
            fresh.style_at(at),
            "the style at {at} differs"
        );
    }
}
