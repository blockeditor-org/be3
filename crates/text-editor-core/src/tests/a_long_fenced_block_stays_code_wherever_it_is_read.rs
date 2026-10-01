use super::*;

#[test]
fn a_long_fenced_block_stays_code_wherever_it_is_read() {
    let mut text = String::from("intro\n\n```\n");
    for line in 0..2000 {
        text.push_str(&format!("# not a heading {line}\n\n"));
    }
    text.push_str("```\n\n# A heading\n");
    let mut tester = EditorTester::with_language(text.as_bytes(), TextLanguage::Markdown);
    let highlight = tester.editor.highlight();

    for line in [5, 900, 1999] {
        let at = text.find(&format!("# not a heading {line}\n")).unwrap() + 2;
        assert_eq!(
            highlight.style_at(at).color,
            SynHlColorScope::MarkdownCode,
            "line {line} of the fence was not read as code"
        );
    }
    let heading = text.find("A heading").unwrap();
    assert!(matches!(
        highlight.style_at(heading).size,
        SynHlTextSize::Heading(1)
    ));
    assert!(highlight.in_code_block(text.find("# not a heading 1500").unwrap()..text.len()));
    assert!(!highlight.in_code_block(heading..heading + 1));
}
