use super::*;
use crate::highlighter::{Language, decorations};

#[test]
fn markdown_code_blocks_are_highlighted_by_their_language() {
    let source = b"```rust,ignore\nfn main() {}\n```\n\n```TOML\nkey = \"value\"\n```\n\n```text\nfn main() {}\n```\n";
    let mut tester = EditorTester::with_language(source, TextLanguage::Markdown);
    let highlight = tester.editor.highlight();
    let at = |needle: &[u8], from: usize| {
        from + source[from..]
            .windows(needle.len())
            .position(|window| window == needle)
            .expect("the source holds it")
    };

    let rust = at(b"fn", 0);
    let rust_expected = decorations(Language::Rust, b"fn main() {}\n");
    for offset in 0..12 {
        assert_eq!(
            highlight.style_at(rust + offset).color,
            rust_expected[offset].color
        );
        assert_eq!(
            highlight.style_at(rust + offset).family,
            SynHlFontFamily::Monospace
        );
    }
    assert_ne!(
        highlight.style_at(rust).color,
        SynHlColorScope::MarkdownCode
    );

    let toml = at(b"key", 0);
    let toml_expected = decorations(Language::Toml, b"key = \"value\"\n");
    assert_eq!(
        highlight.style_at(at(b"\"value\"", toml)).color,
        toml_expected[6].color
    );
    assert_ne!(
        highlight.style_at(at(b"\"value\"", toml)).color,
        SynHlColorScope::MarkdownCode
    );

    let plain = at(b"fn", at(b"```text", 0));
    assert_eq!(
        highlight.style_at(plain).color,
        SynHlColorScope::MarkdownCode
    );
}
