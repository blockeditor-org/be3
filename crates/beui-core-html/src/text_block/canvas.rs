use beui_types::text::FontId;

use crate::css;

pub(super) fn width(text: &str, font: FontId) -> Option<f32> {
    crate::text::canvas(|context| {
        context.set_font(&css::font(font));
        context
            .measure_text(text)
            .ok()
            .map(|metrics| metrics.width() as f32)
    })
    .flatten()
}

pub(super) fn extents(font: FontId) -> Option<(f32, f32)> {
    crate::text::canvas(|context| {
        context.set_font(&css::font(font));
        context.measure_text("x").ok().map(|metrics| {
            (
                metrics.font_bounding_box_ascent() as f32,
                metrics.font_bounding_box_descent() as f32,
            )
        })
    })
    .flatten()
}

pub(super) fn covers(_text: &str, _font: FontId) -> bool {
    true
}
