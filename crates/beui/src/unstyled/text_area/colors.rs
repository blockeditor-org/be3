use text_editor_core::SynHlColorScope;

use crate::color::Color32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SyntaxColors {
    pub invalid: Color32,
    pub keyword: Color32,
    pub keyword_storage: Color32,
    pub keyword_primitive_type: Color32,
    pub literal: Color32,
    pub literal_string: Color32,
    pub comment: Color32,
    pub punctuation: Color32,
    pub punctuation_important: Color32,
    pub variable: Color32,
    pub variable_constant: Color32,
    pub variable_function: Color32,
    pub variable_mutable: Color32,
    pub variable_parameter: Color32,
    pub markdown_plain_text: Color32,
    pub markdown_symbol: Color32,
    pub markdown_link: Color32,
    pub markdown_code: Color32,
    pub unstyled: Color32,
    pub invisible: Color32,
}

impl SyntaxColors {
    pub const DEFAULT: Self = Self {
        invalid: rgb(0xff0000),
        keyword: rgb(0x5ec4ff),
        keyword_storage: rgb(0x008b94),
        keyword_primitive_type: rgb(0x70e1e8),
        literal: rgb(0xe27e8d),
        literal_string: rgb(0x68a1f0),
        comment: rgb(0xff9d1c),
        punctuation: rgb(0x718ca1),
        punctuation_important: rgb(0xb7c5d3),
        variable: rgb(0x718ca1),
        variable_constant: rgb(0x8bd49c),
        variable_function: rgb(0x70e1e8),
        variable_mutable: rgb(0xb7c5d3),
        variable_parameter: rgb(0xebbf83),
        markdown_plain_text: rgb(0xffffff),
        markdown_symbol: rgb(0x718ca1),
        markdown_link: rgb(0x70e1e8),
        markdown_code: rgb(0x8bd49c),
        unstyled: rgb(0xb7c5d3),
        invisible: rgb(0x43515c),
    };

    pub fn scope(&self, scope: SynHlColorScope) -> Color32 {
        match scope {
            SynHlColorScope::Invalid => self.invalid,
            SynHlColorScope::Keyword => self.keyword,
            SynHlColorScope::KeywordStorage => self.keyword_storage,
            SynHlColorScope::KeywordPrimitiveType => self.keyword_primitive_type,
            SynHlColorScope::Literal => self.literal,
            SynHlColorScope::LiteralString => self.literal_string,
            SynHlColorScope::Comment => self.comment,
            SynHlColorScope::Punctuation => self.punctuation,
            SynHlColorScope::PunctuationImportant => self.punctuation_important,
            SynHlColorScope::Variable => self.variable,
            SynHlColorScope::VariableConstant => self.variable_constant,
            SynHlColorScope::VariableFunction => self.variable_function,
            SynHlColorScope::VariableMutable => self.variable_mutable,
            SynHlColorScope::VariableParameter => self.variable_parameter,
            SynHlColorScope::MarkdownPlainText => self.markdown_plain_text,
            SynHlColorScope::MarkdownSymbol => self.markdown_symbol,
            SynHlColorScope::MarkdownLink => self.markdown_link,
            SynHlColorScope::MarkdownCode => self.markdown_code,
            SynHlColorScope::Unstyled => self.unstyled,
            SynHlColorScope::Invisible => self.invisible,
        }
    }
}

impl Default for SyntaxColors {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextAreaColors {
    pub surface: Color32,
    pub gutter: Color32,
    pub gutter_border: Color32,
    pub gutter_text: Color32,
    pub gutter_arrow: Color32,
    pub code_background: Color32,
    pub revealed_background: Color32,
    pub widget: Color32,
    pub broken_widget: Color32,
    pub selection: Color32,
    pub caret: Color32,
    pub syntax: SyntaxColors,
}

impl TextAreaColors {
    pub const DEFAULT: Self = Self {
        surface: rgb(0x1d252c),
        gutter: rgb(0x181f25),
        gutter_border: rgb(0x28333c),
        gutter_text: rgb(0x718ca1),
        gutter_arrow: rgb(0x8fa8ba),
        code_background: rgb(0x171e24),
        revealed_background: rgb(0x2d2114),
        widget: rgb(0x31414e),
        broken_widget: rgb(0x48373d),
        selection: rgb(0x213054),
        caret: rgb(0x5289ff),
        syntax: SyntaxColors::DEFAULT,
    };
}

impl Default for TextAreaColors {
    fn default() -> Self {
        Self::DEFAULT
    }
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}
