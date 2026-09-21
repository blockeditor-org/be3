use crate::color::Color32;
use crate::context::Context;
use crate::drawing::Drawing;
use crate::font::{FontId, Galley, TextLayout};
use crate::geometry::{Pos2, Rect, Rotation};
use crate::image::Image;
use crate::pixel_grid::PixelGrid;

#[derive(Clone, PartialEq)]
pub enum Shape {
    Rect {
        rect: Rect,
        corner_radius: f32,
        stroke_width: f32,
        color: Color32,
        rotation: Rotation,
        clip: Rect,
    },
    Text {
        origin: Pos2,
        galley: Galley,
        color: Color32,
        rotation: Rotation,
        clip: Rect,
    },
    Image {
        rect: Rect,
        source: Rect,
        image: Image,
        tint: Color32,
        corner_radius: f32,
        smooth: bool,
        rotation: Rotation,
        clip: Rect,
    },
    Line {
        from: Pos2,
        to: Pos2,
        width: f32,
        color: Color32,
        clip: Rect,
    },
    Punch {
        rect: Rect,
        corner_radius: f32,
        rotation: Rotation,
        clip: Rect,
    },
    Drawing {
        rect: Rect,
        drawing: Drawing,
        clip: Rect,
    },
}

pub struct Painter {
    context: Context,
    clip: Rect,
    top: bool,
    rotation: Rotation,
}

impl Painter {
    pub(crate) fn new(context: Context, clip: Rect) -> Self {
        Self {
            context,
            clip,
            top: false,
            rotation: Rotation::NONE,
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
            rotation: self.rotation,
        }
    }

    pub fn on_top(&self) -> Self {
        Self {
            context: self.context.clone(),
            clip: self.clip,
            top: true,
            rotation: self.rotation,
        }
    }

    pub fn rotated(&self, pivot: Pos2, angle: f32) -> Self {
        let rotation = match angle == 0.0 {
            true => Rotation::NONE,
            false => Rotation::new(pivot, angle),
        };
        Self {
            context: self.context.clone(),
            clip: self.clip,
            top: self.top,
            rotation,
        }
    }

    pub fn rotation(&self) -> Rotation {
        self.rotation
    }

    fn push(&self, shape: Shape) {
        if self.top {
            self.context.push_top(shape);
        } else {
            self.context.push(shape);
        }
    }

    pub fn layout(&self, text: impl Into<String>, font: FontId, wrap_width: f32) -> Galley {
        self.context
            .layout(&text.into(), font, TextLayout::wrapped(wrap_width))
    }

    pub fn layout_text(&self, text: impl Into<String>, font: FontId, layout: TextLayout) -> Galley {
        self.context.layout(&text.into(), font, layout)
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
            rotation: self.rotation,
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
            rotation: self.rotation,
            clip: self.clip,
        });
    }

    pub fn line(&self, from: Pos2, to: Pos2, width: f32, color: Color32) {
        if color.alpha() == 0 || width <= 0.0 {
            return;
        }
        self.push(Shape::Line {
            from: self.rotation.apply(from),
            to: self.rotation.apply(to),
            width,
            color,
            clip: self.clip,
        });
    }

    pub fn image(
        &self,
        rect: Rect,
        source: Rect,
        image: &Image,
        tint: Color32,
        corner_radius: f32,
        smooth: bool,
    ) {
        if tint.alpha() == 0 || !rect.is_positive() || image.width() == 0 || image.height() == 0 {
            return;
        }
        self.push(Shape::Image {
            rect,
            source,
            image: image.clone(),
            tint,
            corner_radius,
            smooth,
            rotation: self.rotation,
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
            rotation: self.rotation,
            clip: self.clip,
        });
    }

    pub fn drawing(&self, rect: Rect, drawing: &Drawing) {
        if !rect.is_positive() {
            return;
        }
        self.push(Shape::Drawing {
            rect,
            drawing: drawing.clone(),
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
            rotation: self.rotation,
            clip: self.clip,
        });
    }

    pub fn text(&self, origin: Pos2, text: impl Into<String>, font: FontId, color: Color32) {
        let galley = self.layout(text, font, f32::INFINITY);
        self.galley(origin, galley, color);
    }
}
