use text_editor_core::SynHlColorScope;

use beui_view::types::Color32;

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
    pub const fn uniform(color: Color32) -> Self {
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextAreaColors {
    pub gutter_border: Color32,
    pub gutter_text: Color32,
    pub gutter_arrow: Color32,
    pub code_background: Color32,
    pub widget: Color32,
    pub broken_widget: Color32,
    pub selection: Color32,
    pub caret: Color32,
    pub placeholder: Color32,
    pub remote_selection_alpha: u8,
    pub syntax: SyntaxColors,
}
