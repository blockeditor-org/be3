use super::*;

#[test]
fn markdown_styles_hold_across_style_windows() {
    let mut text = "word ".repeat(819);
    text.push_str("**bold across the edge** and `code across` it\n");
    text.push_str("# A heading\n");
    let bold = text.find("**bold").unwrap();
    assert!(
        bold < 4096 && bold + 20 > 4096,
        "the bold run must straddle a window"
    );
    let mut tester = EditorTester::with_language(text.as_bytes(), TextLanguage::Markdown);
    let highlight = tester.editor.highlight();

    for index in bold + 2..bold + 22 {
        assert!(highlight.style_at(index).bold, "byte {index} lost its bold");
    }
    assert_eq!(
        highlight.style_at(bold).color,
        SynHlColorScope::MarkdownSymbol
    );
    let code = text.find("`code").unwrap();
    assert_eq!(
        highlight.style_at(code + 1).color,
        SynHlColorScope::MarkdownCode
    );
    assert!(!highlight.style_at(bold - 1).bold);
    let heading = text.find("A heading").unwrap();
    assert!(matches!(
        highlight.style_at(heading).size,
        SynHlTextSize::Heading(1)
    ));
    assert_eq!(
        highlight.style_at(text.len()).color,
        SynHlColorScope::Invalid
    );
}
