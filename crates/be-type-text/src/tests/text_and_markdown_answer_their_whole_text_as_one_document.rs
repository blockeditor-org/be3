use be_block::{BlockContent, Stored};
use be_sdk_type_guest::api::{Loaded, SearchDocument, guest::exports::replicas::Guest};

use super::*;

fn search(content: &TextContent, keys: Option<Vec<String>>) -> Vec<SearchDocument> {
    let loaded = Loaded::of(Stored::unsaved(content.encode()));
    pollster::block_on(crate::block_types().search_text(
        TextContent::CONTENT_TYPE,
        loaded,
        keys,
        Vec::new(),
    ))
    .expect("the text answers")
}

#[test]
fn text_and_markdown_answer_their_whole_text_as_one_document() {
    for language in [TextLanguage::PlainText, TextLanguage::Markdown] {
        let source = "# Groceries\n\nOat milk and *rye* bread.\n";
        let content = TextBlock::new(source).with_language(language).content();
        assert_eq!(
            search(&content, None),
            vec![SearchDocument {
                key: String::new(),
                source: None,
                name: None,
                body: Some(source.to_owned()),
            }],
            "{language:?} is one document of its whole text"
        );
        assert_eq!(
            search(&content, Some(vec![String::new()])).len(),
            1,
            "asked for its one key, it answers it"
        );
        assert_eq!(
            search(&content, Some(vec!["other".to_owned()])),
            Vec::new(),
            "asked for another key, it answers nothing"
        );
    }
}
