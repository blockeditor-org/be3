use super::*;

#[test]
fn rich_text_a_code_block_is_decorated_by_its_language() {
    let mut tester = EditorTester::rich(b"Code:\nfn main() {}\nlet x = 1;");
    select(&mut tester, 7..20);
    tool(&mut tester, "code_block");
    assert_eq!(
        formatted(&tester),
        "Code:\n[code_block=rust]fn main() {}\n[code_block=rust]let x = 1;"
    );
    let highlight = tester.editor.highlight();
    assert_eq!(highlight.style_at(0).family, SynHlFontFamily::Proportional);
    let keyword = highlight.style_at(6);
    assert_eq!(keyword.family, SynHlFontFamily::Monospace);
    assert_eq!(
        keyword.color,
        SynHlColorScope::KeywordStorage,
        "the rust lexer decorates the code block"
    );
    assert_ne!(highlight.style_at(9).color, keyword.color);
}
