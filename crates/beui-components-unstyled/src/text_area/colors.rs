use text_editor_core::SynHlColorScope;

use beui_view::types::Color;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SyntaxColors {
    pub invalid: Color,
    pub keyword: Color,
    pub keyword_storage: Color,
    pub keyword_primitive_type: Color,
    pub literal: Color,
    pub literal_string: Color,
    pub comment: Color,
    pub punctuation: Color,
    pub punctuation_important: Color,
    pub variable: Color,
    pub variable_constant: Color,
    pub variable_function: Color,
    pub variable_mutable: Color,
    pub variable_parameter: Color,
    pub markdown_plain_text: Color,
    pub markdown_symbol: Color,
    pub markdown_link: Color,
    pub markdown_code: Color,
    pub unstyled: Color,
    pub invisible: Color,
}

impl SyntaxColors {
    pub const fn uniform(color: Color) -> Self {
        Self {
            invalid: color,
            keyword: color,
            keyword_storage: color,
            keyword_primitive_type: color,
            literal: color,
            literal_string: color,
            comment: color,
            punctuation: color,
            punctuation_important: color,
            variable: color,
            variable_constant: color,
            variable_mutable: color,
            variable_function: color,
            variable_parameter: color,
            markdown_plain_text: color,
            markdown_symbol: color,
            markdown_link: color,
            markdown_code: color,
            unstyled: color,
            invisible: color,
        }
    }

    pub fn scope(&self, scope: SynHlColorScope) -> Color {
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextAreaColors {
    pub gutter_border: Color,
    pub gutter_text: Color,
    pub gutter_arrow: Color,
    pub code_background: Color,
    pub widget: Color,
    pub broken_widget: Color,
    pub selection: Color,
    pub caret: Color,
    pub placeholder: Color,
    pub remote_selection_alpha: u8,
    pub syntax: SyntaxColors,
}
