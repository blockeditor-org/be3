use beui::Color32;
use text_editor_core::SynHlColorScope;

pub(crate) const SURFACE: Color32 = Color32::from_rgb(29, 37, 44);
pub(crate) const GUTTER: Color32 = Color32::from_rgb(24, 31, 37);
pub(crate) const GUTTER_BORDER: Color32 = Color32::from_rgb(40, 51, 60);
pub(crate) const GUTTER_TEXT: Color32 = Color32::from_rgb(0x71, 0x8c, 0xa1);
pub(crate) const GUTTER_ARROW: Color32 = Color32::from_rgb(0x8f, 0xa8, 0xba);
pub(crate) const CODE_BACKGROUND: Color32 = Color32::from_rgb(23, 30, 36);
pub(crate) const REVEALED_BACKGROUND: Color32 = Color32::from_rgb(45, 33, 20);
pub(crate) const INLINE_EMBED: Color32 = Color32::from_rgb(49, 65, 78);
pub(crate) const BROKEN_EMBED: Color32 = Color32::from_rgb(72, 55, 61);

pub(crate) fn syntax(scope: SynHlColorScope) -> Color32 {
    let hex = match scope {
        SynHlColorScope::Invalid => 0xff0000,
        SynHlColorScope::KeywordStorage => 0x008b94,
        SynHlColorScope::Literal => 0xe27e8d,
        SynHlColorScope::VariableFunction => 0x70e1e8,
        SynHlColorScope::PunctuationImportant => 0xb7c5d3,
        SynHlColorScope::Variable => 0x718ca1,
        SynHlColorScope::VariableParameter => 0xebbf83,
        SynHlColorScope::LiteralString => 0x68a1f0,
        SynHlColorScope::KeywordPrimitiveType => 0x70e1e8,
        SynHlColorScope::Punctuation => 0x718ca1,
        SynHlColorScope::Keyword => 0x5ec4ff,
        SynHlColorScope::VariableConstant => 0x8bd49c,
        SynHlColorScope::VariableMutable => 0xb7c5d3,
        SynHlColorScope::Comment => 0xff9d1c,
        SynHlColorScope::MarkdownPlainText => 0xffffff,
        SynHlColorScope::MarkdownSymbol => 0x718ca1,
        SynHlColorScope::MarkdownLink => 0x70e1e8,
        SynHlColorScope::MarkdownCode => 0x8bd49c,
        SynHlColorScope::Unstyled => 0xb7c5d3,
        SynHlColorScope::Invisible => 0x43515c,
    };
    Color32::from_rgb(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
    )
}
