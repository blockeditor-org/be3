use crate::color::Color32;
use crate::context::Context;
use crate::drawing::Drawing;
use crate::fade::Fade;
use crate::font::{FontId, Galley, TextLayout};
use crate::geometry::{Pos2, Rect, Rotation, Vec2};
use crate::image::Image;
use crate::node::{NodeId, SpaceId};
use crate::pixel_grid::PixelGrid;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Corners {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl Corners {
    pub fn all(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    pub fn to_array(self) -> [f32; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }

    pub fn largest(self) -> f32 {
        self.to_array().into_iter().fold(0.0, f32::max)
    }

    pub fn scaled(self, scale: f32) -> Self {
        let [top_left, top_right, bottom_right, bottom_left] =
            self.to_array().map(|radius| radius * scale);
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }
}

impl From<f32> for Corners {
    fn from(radius: f32) -> Self {
        Self::all(radius)
    }
}

#[derive(Clone, PartialEq)]
pub enum Shape {
    Rect {
        rect: Rect,
        corner_radius: Corners,
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

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Entry {
    pub translation: Vec2,
    pub clip: Rect,
    pub shift: Option<Vec2>,
    pub fade: Fade,
}

impl Entry {
    pub const NONE: Self = Self {
        translation: Vec2::ZERO,
        clip: Rect::EVERYTHING,
        shift: None,
        fade: Fade::NONE,
    };

    pub fn place(self, rect: Rect) -> Rect {
        rect.translate(self.translation).intersect(self.clip)
    }

    pub fn compose(self, inner: Self) -> Self {
        Self {
            translation: self.translation + inner.translation,
            clip: self.clip.intersect(inner.clip.translate(self.translation)),
            shift: match inner.shift {
                Some(shift) => Some(self.translation + shift),
                None => self.shift,
            },
            fade: self.fade.within(inner.fade.translate(self.translation)),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PainterState {
    pub clip: Rect,
    pub space_clip: Rect,
    pub origin: Vec2,
    pub space: Option<SpaceId>,
    pub top: bool,
    pub rotation: Rotation,
    pub list: Entry,
}

impl PainterState {
    pub fn settles(self, other: Self) -> bool {
        self.clip == other.clip
            && self.space == other.space
            && self.top == other.top
            && self.rotation == other.rotation
    }

    pub fn sees(self, other: Self) -> bool {
        self.space_clip == other.space_clip && self.origin == other.origin
    }

    pub fn resumed(self, own: Self) -> Self {
        Self {
            origin: own.origin + self.list.translation,
            space_clip: own
                .space_clip
                .intersect(self.list.clip)
                .translate(-self.list.translation),
            ..self
        }
    }
}

pub struct Painter {
    context: Context,
    clip: Rect,
    space_clip: Rect,
    origin: Vec2,
    space: Option<SpaceId>,
    top: bool,
    rotation: Rotation,
    list: Entry,
}

impl Painter {
    pub fn new(context: Context, clip: Rect) -> Self {
        Self {
            context,
            clip,
            space_clip: Rect::EVERYTHING,
            origin: Vec2::ZERO,
            space: None,
            top: false,
            rotation: Rotation::NONE,
            list: Entry::NONE,
        }
    }

    pub fn resumed(context: Context, state: PainterState) -> Self {
        Self {
            context,
            clip: state.clip,
            space_clip: state.space_clip,
            origin: state.origin,
            space: state.space,
            top: state.top,
            rotation: state.rotation,
            list: state.list,
        }
    }

    pub fn state(&self) -> PainterState {
        PainterState {
            clip: self.clip,
            space_clip: self.space_clip,
            origin: self.origin,
            space: self.space,
            top: self.top,
            rotation: self.rotation,
            list: self.list,
        }
    }

    fn with(&self, clip: Rect, top: bool, rotation: Rotation) -> Self {
        Self {
            context: self.context.clone(),
            clip,
            space_clip: self.space_clip,
            origin: self.origin,
            space: self.space,
            top,
            rotation,
            list: self.list,
        }
    }

    pub fn entered(&self, node: NodeId, rect: Rect) -> Self {
        let translation = rect.min.to_vec2();
        Self {
            context: self.context.clone(),
            clip: Rect::EVERYTHING,
            space_clip: self.space_clip.intersect(self.clip).translate(-translation),
            origin: self.origin + translation,
            space: Some(SpaceId::of(node)),
            top: self.top,
            rotation: self.rotation.translate(-translation),
            list: Entry::NONE,
        }
    }

    pub fn entry(&self, rect: Rect) -> Entry {
        Entry {
            translation: self.list.translation + rect.min.to_vec2(),
            clip: self
                .list
                .clip
                .intersect(self.clip.translate(self.list.translation)),
            shift: self.list.shift,
            fade: self.list.fade,
        }
    }

    pub fn shifted(&self, space: Option<SpaceId>, by: Vec2, clip: Rect) -> Self {
        let kept = self.clip.intersect(clip);
        Self {
            context: self.context.clone(),
            clip: Rect::EVERYTHING,
            space_clip: self.space_clip.intersect(kept).translate(-by),
            origin: self.origin + by,
            space,
            top: self.top,
            rotation: self.rotation.translate(-by),
            list: Entry {
                translation: self.list.translation + by,
                clip: self
                    .list
                    .clip
                    .intersect(kept.translate(self.list.translation)),
                shift: Some(self.list.shift.unwrap_or(Vec2::ZERO) + by),
                fade: self.list.fade,
            },
        }
    }

    pub fn faded(&self, rect: Rect, widths: [f32; 4]) -> Self {
        let fade = Fade::new(rect, widths).translate(self.list.translation);
        Self {
            context: self.context.clone(),
            list: Entry {
                fade: self.list.fade.within(fade),
                ..self.list
            },
            ..*self
        }
    }

    pub fn in_document(&self) -> Self {
        self.context.note_space_read();
        let mut painter = self.shifted(None, -self.origin, Rect::EVERYTHING);
        painter.list.shift = self.list.shift;
        painter
    }

    pub fn ctx(&self) -> &Context {
        &self.context
    }

    pub fn clip_rect(&self) -> Rect {
        self.context.note_space_read();
        self.clip.intersect(self.space_clip)
    }

    pub fn origin(&self) -> Vec2 {
        self.context.note_space_read();
        self.origin
    }

    pub fn pixel_grid(&self) -> PixelGrid {
        PixelGrid::new(self.context.pixels_per_point())
    }

    pub fn with_clip_rect(&self, clip: Rect) -> Self {
        self.with(self.clip.intersect(clip), self.top, self.rotation)
    }

    pub fn on_top(&self) -> Self {
        self.with(self.clip, true, self.rotation)
    }

    pub fn rotated(&self, pivot: Pos2, angle: f32) -> Self {
        let rotation = match angle == 0.0 {
            true => Rotation::NONE,
            false => Rotation::new(pivot, angle),
        };
        self.with(self.clip, self.top, rotation)
    }

    pub fn rotation(&self) -> Rotation {
        self.rotation
    }

    fn push(&self, shape: Shape) {
        let shape = match self.list == Entry::NONE {
            true => shape,
            false => placed_shape(&shape, self.list.translation, self.list.clip),
        };
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

    pub fn rect_filled(&self, rect: Rect, corner_radius: impl Into<Corners>, color: Color32) {
        if color.alpha() == 0 {
            return;
        }
        self.push(Shape::Rect {
            rect,
            corner_radius: corner_radius.into(),
            stroke_width: 0.0,
            color,
            rotation: self.rotation,
            clip: self.clip,
        });
    }

    pub fn rect_stroke(
        &self,
        rect: Rect,
        corner_radius: impl Into<Corners>,
        width: f32,
        color: Color32,
    ) {
        if color.alpha() == 0 || width <= 0.0 {
            return;
        }
        self.push(Shape::Rect {
            rect,
            corner_radius: corner_radius.into(),
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
        if color.alpha() == 0 || galley.is_blank() {
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

pub fn placed_shape(shape: &Shape, offset: Vec2, space_clip: Rect) -> Shape {
    let mut shape = shape.clone();
    match &mut shape {
        Shape::Rect {
            rect,
            rotation,
            clip,
            ..
        }
        | Shape::Image {
            rect,
            rotation,
            clip,
            ..
        }
        | Shape::Punch {
            rect,
            rotation,
            clip,
            ..
        } => {
            *rect = rect.translate(offset);
            *rotation = rotation.translate(offset);
            *clip = clip.translate(offset).intersect(space_clip);
        }
        Shape::Text {
            origin,
            rotation,
            clip,
            ..
        } => {
            *origin += offset;
            *rotation = rotation.translate(offset);
            *clip = clip.translate(offset).intersect(space_clip);
        }
        Shape::Line { from, to, clip, .. } => {
            *from += offset;
            *to += offset;
            *clip = clip.translate(offset).intersect(space_clip);
        }
        Shape::Drawing { rect, clip, .. } => {
            *rect = rect.translate(offset);
            *clip = clip.translate(offset).intersect(space_clip);
        }
    }
    shape
}
