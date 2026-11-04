use beui_core::current::try_with_document;
use beui_types::{FontId, TextLayout};

pub(crate) use beui_core::font::Galley as TextBlock;
pub(crate) use beui_core::painter::Painter;

pub(crate) fn lay_out_text(text: &str, font: FontId, layout: TextLayout) -> Option<TextBlock> {
    try_with_document(|document| document.layout_text(text, font, layout)).flatten()
}

pub(crate) fn lay_out(
    painter: &Painter,
    text: &str,
    font: FontId,
    layout: TextLayout,
) -> TextBlock {
    painter.layout_text(text, font, layout)
}

pub(crate) fn draw_text(
    painter: &Painter,
    origin: beui_types::Pos2,
    block: &TextBlock,
    color: beui_types::Color,
) {
    painter.galley(origin, block.clone(), painter.resolve_color(color));
}
