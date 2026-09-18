use crate::color::Color32;
use crate::context::Context;
use crate::font::{FontId, Galley};
use crate::geometry::{Pos2, Rect};
use crate::pixel_grid::PixelGrid;

#[derive(Clone, PartialEq)]
pub enum Shape {
    Rect {
        rect: Rect,
        corner_radius: f32,
        stroke_width: f32,
        color: Color32,
        clip: Rect,
    },
    Text {
        origin: Pos2,
        galley: Galley,
        color: Color32,
        clip: Rect,
    },
    Punch {
        rect: Rect,
        corner_radius: f32,
        clip: Rect,
    },
}

pub struct Painter {
    context: Context,
    clip: Rect,
    top: bool,
}

impl Painter {
    pub(crate) fn new(context: Context, clip: Rect) -> Self {
        Self {
            context,
            clip,
            top: false,
        }
    }

    pub fn ctx(&self) -> &Context {
        &self.context
    }

    pub fn clip_rect(&self) -> Rect {
        self.clip
    }

    pub(crate) fn pixel_grid(&self) -> PixelGrid {
        PixelGrid::new(self.context.pixels_per_point())
    }

    pub fn with_clip_rect(&self, clip: Rect) -> Self {
        Self {
            context: self.context.clone(),
            clip: self.clip.intersect(clip),
            top: self.top,
        }
    }

    pub fn on_top(&self) -> Self {
        Self {
            context: self.context.clone(),
            clip: self.clip,
            top: true,
        }
    }

    fn push(&self, shape: Shape) {
        if self.top {
            self.context.push_top(shape);
        } else {
            self.context.push(shape);
        }
    }

    pub fn layout(&self, text: impl Into<String>, font: FontId, wrap_width: f32) -> Galley {
        self.context.layout(&text.into(), font, wrap_width)
    }

    pub fn rect_filled(&self, rect: Rect, corner_radius: f32, color: Color32) {
        if color.alpha() == 0 {
            return;
        }
        self.push(Shape::Rect {
            rect,
            corner_radius,
            stroke_width: 0.0,
            color,
            clip: self.clip,
        });
    }

    pub fn rect_stroke(&self, rect: Rect, corner_radius: f32, width: f32, color: Color32) {
        if color.alpha() == 0 || width <= 0.0 {
            return;
        }
        self.push(Shape::Rect {
            rect,
            corner_radius,
            stroke_width: width,
            color,
            clip: self.clip,
        });
    }

    pub fn punch(&self, rect: Rect, corner_radius: f32) {
        if !rect.is_positive() {
            return;
        }
        self.push(Shape::Punch {
            rect,
            corner_radius,
            clip: self.clip,
        });
    }

    pub fn galley(&self, origin: Pos2, galley: Galley, color: Color32) {
        if color.alpha() == 0 || galley.glyphs().is_empty() {
            return;
        }
        self.push(Shape::Text {
            origin,
            galley,
            color,
            clip: self.clip,
        });
    }

    pub fn text(&self, origin: Pos2, text: impl Into<String>, font: FontId, color: Color32) {
        let galley = self.layout(text, font, f32::INFINITY);
        self.galley(origin, galley, color);
    }
}
