use std::collections::HashSet;

use be_block_content::{BlockContent, PartReader, SearchDocument};

use super::*;

#[test]
fn text_in_any_language_answers_its_whole_text_as_one_document() {
    let source = "# Groceries\n\nOat milk and *rye* bread.\n";
    for language in [
        TextLanguage::Markdown,
        TextLanguage::PlainText,
        TextLanguage::Rust,
        TextLanguage::Zig,
        TextLanguage::Toml,
    ] {
        let content = TextBlock::new(source).with_language(language).content();
        let parts = PartReader::new(|_, _, _| panic!("text is loaded whole and reads no part"));
        assert_eq!(
            pollster::block_on(content.search_text(None, &HashSet::new(), parts)),
            Ok(vec![SearchDocument {
                key: String::new(),
                source: None,
                name: None,
                body: Some(source.to_owned()),
            }]),
            "{language:?} is one document of its whole text"
        );
    }
}
