use std::rc::Rc;

use crate::types::path::{FillRule, Path, Stroke};
use crate::types::pixel_grid::PixelGrid;
use crate::types::{Color, Color32, Corners, FontId, Image, LineBox, Pos2, Rect, TextLayout, Vec2};

pub type Draw = Rc<dyn Fn(&Painter, Rect)>;

pub struct Painter(crate::core::painter::Painter);

impl Painter {
    pub(crate) fn of(painter: &crate::core::painter::Painter) -> Painter {
        Painter(painter.clone())
    }

    #[cfg(not(beui_html))]
    pub(crate) fn core(&self) -> &crate::core::painter::Painter {
        &self.0
    }

    pub fn pixel_grid(&self) -> PixelGrid {
        self.0.pixel_grid()
    }

    pub fn resolve_color(&self, color: Color) -> Color32 {
        self.0.resolve_color(color)
    }

    pub fn clip_rect(&self) -> Rect {
        self.0.clip_rect()
    }

    pub fn transformed(&self, offset: Vec2, scale: f32) -> Painter {
        Painter(self.0.transformed(offset, scale))
    }

    pub fn rotated(&self, pivot: Pos2, angle: f32) -> Painter {
        Painter(self.0.rotated(pivot, angle))
    }

    pub fn with_clip_rect(&self, clip: Rect) -> Painter {
        Painter(self.0.with_clip_rect(clip))
    }

    pub fn rect_filled(
        &self,
        rect: Rect,
        corner_radius: impl Into<Corners>,
        color: impl Into<Color>,
    ) {
        self.0.rect_filled(rect, corner_radius, color.into());
    }

    pub fn rect_stroke(
        &self,
        rect: Rect,
        corner_radius: impl Into<Corners>,
        width: f32,
        color: impl Into<Color>,
    ) {
        self.0.rect_stroke(rect, corner_radius, width, color.into());
    }

    pub fn line(&self, from: Pos2, to: Pos2, width: f32, color: impl Into<Color>) {
        self.0.line(from, to, width, color.into());
    }

    pub fn image(
        &self,
        rect: Rect,
        source: Rect,
        image: &Image,
        tint: impl Into<Color>,
        corner_radius: f32,
        smooth: bool,
    ) {
        self.0
            .image(rect, source, image, tint.into(), corner_radius, smooth);
    }

    pub fn fill_path(&self, path: &Path, rule: FillRule, color: impl Into<Color>) {
        self.0.fill_path(path, rule, color.into());
    }

    pub fn stroke_path(&self, path: &Path, stroke: &Stroke, color: impl Into<Color>) {
        self.0.stroke_path(path, stroke, color.into());
    }

    pub fn lay_out_text(&self, text: &str, font: FontId, layout: TextLayout) -> TextBlock {
        TextBlock(crate::core::painter::lay_out(&self.0, text, font, layout))
    }

    pub fn draw_text(&self, origin: Pos2, text: &TextBlock, color: impl Into<Color>) {
        crate::core::painter::draw_text(&self.0, origin, &text.0, color.into());
    }
}

#[derive(Clone)]
pub struct TextBlock(pub(crate) crate::core::painter::TextBlock);

impl TextBlock {
    pub fn text(&self) -> &str {
        self.0.text()
    }

    pub fn font(&self) -> FontId {
        self.0.font()
    }

    pub fn size(&self) -> Vec2 {
        self.0.size()
    }

    pub fn line_height(&self) -> f32 {
        self.0.line_height()
    }

    pub fn baseline(&self) -> f32 {
        self.0.baseline()
    }

    pub fn lines(&self) -> Vec<LineBox> {
        self.0.line_boxes()
    }

    pub fn has_every_glyph(&self) -> bool {
        self.0.has_every_glyph()
    }
}
