use be_model::Document;
use be_model::schema::check_frozen;

use super::frozen_text;
use crate::{TextBlock, TextIndentation, TextLanguage};

fn bytes(hex: &str) -> Vec<u8> {
    let hex: String = hex.split_whitespace().collect();
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
        .collect()
}

#[test]
fn text_saved_before_it_was_frozen_still_loads() {
    assert_eq!(
        check_frozen::<TextBlock>(frozen_text::REGISTRY, frozen_text::DOCUMENT),
        Ok(())
    );
    let text = Document::<TextBlock>::from_bytes(&bytes(frozen_text::DOCUMENT)).unwrap();
    assert_eq!(
        TextBlock::text(&text),
        "fn main() {\n\tprintln!(\"héllo, 世界\");\n}\n"
    );
    assert_eq!(text.root().language, TextLanguage::Rust);
    assert_eq!(text.root().indentation, TextIndentation::Tabs);
}
