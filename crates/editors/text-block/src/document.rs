use block_editor_beui::be_block::be_model::{Document as Content, Edit, ObjectId};
use block_editor_beui::be_block::block_url::{BLOCK_URL_MAX_BYTES, parse_block_urls};
use block_editor_beui::be_block::{self, TextBlock};
use block_editor_beui::live_text::{LiveText, TextField};
use text_editor_core::{TextIndentation, TextLanguage};
use uuid::Uuid;

pub use block_editor_beui::live_text::History;

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

pub const fn block_language(language: TextLanguage) -> be_block::TextLanguage {
    match language {
        TextLanguage::Markdown | TextLanguage::RichText => be_block::TextLanguage::Markdown,
        TextLanguage::PlainText => be_block::TextLanguage::PlainText,
        TextLanguage::Rust => be_block::TextLanguage::Rust,
        TextLanguage::Zig => be_block::TextLanguage::Zig,
        TextLanguage::Toml => be_block::TextLanguage::Toml,
    }
}

pub const fn editor_language(language: be_block::TextLanguage) -> TextLanguage {
    match language {
        be_block::TextLanguage::Markdown => TextLanguage::Markdown,
        be_block::TextLanguage::PlainText => TextLanguage::PlainText,
        be_block::TextLanguage::Rust => TextLanguage::Rust,
        be_block::TextLanguage::Zig => TextLanguage::Zig,
        be_block::TextLanguage::Toml => TextLanguage::Toml,
    }
}

pub const fn block_indentation(indentation: TextIndentation) -> be_block::TextIndentation {
    match indentation {
        TextIndentation::Tabs => be_block::TextIndentation::Tabs,
        TextIndentation::Spaces { width } => be_block::TextIndentation::Spaces { width },
    }
}

pub const fn editor_indentation(indentation: be_block::TextIndentation) -> TextIndentation {
    match indentation {
        be_block::TextIndentation::Tabs => TextIndentation::Tabs,
        be_block::TextIndentation::Spaces { width } => TextIndentation::Spaces { width },
    }
}
