use block_editor_beui::be_block::be_model::{Document as Content, Edit, ObjectId};
use block_editor_beui::be_block::block_url::{BLOCK_URL_MAX_BYTES, parse_block_urls};

use block_editor_beui::live_text::{LiveText, TextField};
use text_editor_core::{TextIndentation, TextLanguage};
use uuid::Uuid;

pub use block_editor_beui::live_text::History;
use text_block_types::TextBlock;

#[cfg(test)]
mod tests;

pub struct TextBody;

pub type BlockDocument = LiveText<TextBody>;

impl TextField for TextBody {
    type Root = TextBlock;

    fn object(&self) -> ObjectId {
        ObjectId::ROOT
    }

    fn field(&self) -> u16 {
        TextBlock::BODY.index()
    }

    fn settings(&self, content: &Content<TextBlock>) -> Option<(TextLanguage, TextIndentation)> {
        Some((
            editor_language(content.field(ObjectId::ROOT, TextBlock::LANGUAGE)),
            editor_indentation(content.field(ObjectId::ROOT, TextBlock::INDENTATION)),
        ))
    }

    fn set_language(&self, language: TextLanguage) -> Option<Edit> {
        Some(TextBlock::set_language(block_language(language)))
    }

    fn set_indentation(&self, indentation: TextIndentation) -> Option<Edit> {
        Some(TextBlock::set_indentation(block_indentation(indentation)))
    }
}

pub fn reference_ranges(bytes: &[u8], reference: Uuid) -> Vec<std::ops::Range<usize>> {
    let length = reference.to_string().len();
    parse_block_urls(bytes)
        .into_iter()
        .filter(|url| url.block == reference)
        .map(|url| url.range.end - length..url.range.end)
        .collect()
}

pub fn inside_block_url(bytes: &[u8], index: usize) -> bool {
    let start = index.saturating_sub(BLOCK_URL_MAX_BYTES);
    let end = (index + BLOCK_URL_MAX_BYTES).min(bytes.len());
    parse_block_urls(&bytes[start..end])
        .iter()
        .any(|url| url.range.start + start < index && index < url.range.end + start)
}

pub const fn block_language(language: TextLanguage) -> text_block_types::TextLanguage {
    match language {
        TextLanguage::Markdown | TextLanguage::RichText => text_block_types::TextLanguage::Markdown,
        TextLanguage::PlainText => text_block_types::TextLanguage::PlainText,
        TextLanguage::Rust => text_block_types::TextLanguage::Rust,
        TextLanguage::Zig => text_block_types::TextLanguage::Zig,
        TextLanguage::Toml => text_block_types::TextLanguage::Toml,
    }
}

pub const fn editor_language(language: text_block_types::TextLanguage) -> TextLanguage {
    match language {
        text_block_types::TextLanguage::Markdown => TextLanguage::Markdown,
        text_block_types::TextLanguage::PlainText => TextLanguage::PlainText,
        text_block_types::TextLanguage::Rust => TextLanguage::Rust,
        text_block_types::TextLanguage::Zig => TextLanguage::Zig,
        text_block_types::TextLanguage::Toml => TextLanguage::Toml,
    }
}

pub const fn block_indentation(indentation: TextIndentation) -> text_block_types::TextIndentation {
    match indentation {
        TextIndentation::Tabs => text_block_types::TextIndentation::Tabs,
        TextIndentation::Spaces { width } => text_block_types::TextIndentation::Spaces { width },
    }
}

pub const fn editor_indentation(indentation: text_block_types::TextIndentation) -> TextIndentation {
    match indentation {
        text_block_types::TextIndentation::Tabs => TextIndentation::Tabs,
        text_block_types::TextIndentation::Spaces { width } => TextIndentation::Spaces { width },
    }
}
