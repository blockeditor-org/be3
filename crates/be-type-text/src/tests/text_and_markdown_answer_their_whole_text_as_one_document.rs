use be_block::{BlockContent, PartReader, SearchDocument};

use super::*;

fn search(content: &TextContent, keys: Option<&[String]>) -> Vec<SearchDocument> {
    let parts = PartReader::new(|_, _, _| panic!("text is loaded whole and reads no part"));
    pollster::block_on(content.search_text(keys, &[], parts)).expect("the text answers")
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
            search(&content, Some(&[String::new()])).len(),
            1,
            "asked for its one key, it answers it"
        );
        assert_eq!(
            search(&content, Some(&["other".to_owned()])),
            Vec::new(),
            "asked for another key, it answers nothing"
        );
    }
}
