use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::color::Color32;
use crate::context::Context;
use crate::display::Group;
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

impl Shape {
    pub fn same_look(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Text {
                    origin,
                    galley,
                    color,
                    rotation,
                    clip,
                },
                Self::Text {
                    origin: other_origin,
                    galley: other_galley,
                    color: other_color,
                    rotation: other_rotation,
                    clip: other_clip,
                },
            ) => {
                origin == other_origin
                    && color == other_color
                    && rotation == other_rotation
                    && clip == other_clip
                    && galley.same_layout(other_galley)
            }
            _ => self == other,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Local {
    pub translation: Vec2,
    pub clip: Rect,
}

impl Local {
    pub const NONE: Self = Self {
        translation: Vec2::ZERO,
        clip: Rect::EVERYTHING,
    };
}

pub struct Scope {
    pub parent: Option<Rc<Scope>>,
    pub group: Group,
    pub id: u64,
}

impl Scope {
    pub fn chain(scope: &Option<Rc<Scope>>) -> Vec<&Scope> {
        let mut chain = Vec::new();
        let mut current = scope.as_deref();
        while let Some(scope) = current {
            chain.push(scope);
            current = scope.parent.as_deref();
        }
        chain.reverse();
        chain
    }
}

static SCOPES: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PainterState {
    pub clip: Rect,
    pub space_clip: Rect,
    pub origin: Vec2,
    pub space: Option<SpaceId>,
    pub scroll: Option<SpaceId>,
    pub rotation: Rotation,
    pub local: Local,
}

impl PainterState {
    pub fn settles(self, other: Self) -> bool {
        self.clip == other.clip && self.space == other.space && self.rotation == other.rotation
    }

    pub fn sees(self, other: Self) -> bool {
        self.space_clip == other.space_clip && self.origin == other.origin
    }

    pub fn resumed(self, own: Self) -> Self {
        Self {
            origin: own.origin + self.local.translation,
            space_clip: own
                .space_clip
                .intersect(self.local.clip)
                .translate(-self.local.translation),
            ..self
        }
    }
}

pub struct Painter {
    context: Context,
    state: PainterState,
    scope: Option<Rc<Scope>>,
}

impl Painter {
    pub fn new(context: Context, clip: Rect) -> Self {
        Self {
            context,
            state: PainterState {
                clip,
                space_clip: Rect::EVERYTHING,
                origin: Vec2::ZERO,
                space: None,
                scroll: None,
                rotation: Rotation::NONE,
                local: Local::NONE,
            },
            scope: None,
        }
    }

    pub fn resumed(context: Context, state: PainterState) -> Self {
        Self {
            context,
            state,
            scope: None,
        }
    }

    pub fn state(&self) -> PainterState {
        self.state
    }

    fn with(&self, state: PainterState) -> Self {
        Self {
            context: self.context.clone(),
            state,
            scope: self.scope.clone(),
        }
    }

    fn grouped(&self, state: PainterState, group: Group) -> Self {
        Self {
            context: self.context.clone(),
            state,
            scope: Some(Rc::new(Scope {
                parent: self.scope.clone(),
                group,
                id: SCOPES.fetch_add(1, Ordering::Relaxed),
            })),
        }
    }

    pub fn entered(&self, node: NodeId, rect: Rect) -> Self {
        let translation = rect.min.to_vec2();
        let state = self.state;
        Self {
            context: self.context.clone(),
            state: PainterState {
                clip: Rect::EVERYTHING,
                space_clip: state
                    .space_clip
                    .intersect(state.clip)
                    .translate(-translation),
                origin: state.origin + translation,
                space: Some(SpaceId::of(node)),
                scroll: state.scroll,
                rotation: state.rotation.translate(-translation),
                local: Local::NONE,
            },
            scope: None,
        }
    }

    pub fn shifted(&self, space: SpaceId, by: Vec2, clip: Rect) -> Self {
        let state = self.state;
        let kept = state.clip.intersect(clip);
        self.grouped(
            PainterState {
                clip: Rect::EVERYTHING,
                space_clip: state.space_clip.intersect(kept).translate(-by),
                origin: state.origin + by,
                space: Some(space),
                scroll: Some(space),
                rotation: state.rotation.translate(-by),
                local: Local {
                    translation: state.local.translation + by,
                    clip: state
                        .local
                        .clip
                        .intersect(kept.translate(state.local.translation)),
                },
            },
            Group::Scroll {
                space,
                viewport: kept,
                offset: by,
            },
        )
    }

    pub fn faded(&self, rect: Rect, widths: [f32; 4]) -> Self {
        self.grouped(self.state, Group::Mask(Fade::new(rect, widths)))
    }

    pub fn in_document(&self) -> Self {
        self.context.note_space_read();
        let state = self.state;
        let by = -state.origin;
        self.grouped(
            PainterState {
                clip: state.clip.translate(-by),
                space_clip: state.space_clip.intersect(state.clip).translate(-by),
                origin: Vec2::ZERO,
                space: None,
                scroll: state.scroll,
                rotation: state.rotation.translate(-by),
                local: Local {
                    translation: state.local.translation + by,
                    clip: state
                        .local
                        .clip
                        .intersect(state.clip.translate(state.local.translation)),
                },
            },
            Group::Transform {
                offset: by,
                scale: 1.0,
            },
        )
    }

    pub fn ctx(&self) -> &Context {
        &self.context
    }

    pub fn clip_rect(&self) -> Rect {
        self.context.note_space_read();
        self.state.clip.intersect(self.state.space_clip)
    }

    pub fn origin(&self) -> Vec2 {
        self.context.note_space_read();
        self.state.origin
    }

    pub fn pixel_grid(&self) -> PixelGrid {
        PixelGrid::new(self.context.pixels_per_point())
    }

    pub fn with_clip_rect(&self, clip: Rect) -> Self {
        self.with(PainterState {
            clip: self.state.clip.intersect(clip),
            ..self.state
        })
    }

    pub fn rotated(&self, pivot: Pos2, angle: f32) -> Self {
        let rotation = match angle == 0.0 {
            true => Rotation::NONE,
            false => Rotation::new(pivot, angle),
        };
        self.with(PainterState {
            rotation,
            ..self.state
        })
    }

    pub fn rotation(&self) -> Rotation {
        self.state.rotation
    }

    pub fn paint_child(&self, id: NodeId, at: Vec2) {
        self.context
            .paint_child(&self.scope, self.state.clip, id, at);
    }

    fn push(&self, shape: Shape) {
        self.context.push_in(&self.scope, shape);
    }

    pub fn layout(&self, text: impl Into<String>, font: FontId, wrap_width: f32) -> Galley {
        self.context
            .layout(&text.into(), font, TextLayout::wrapped(wrap_width))
    }

    pub fn layout_text(&self, text: &str, font: FontId, layout: TextLayout) -> Galley {
        self.context.layout(text, font, layout)
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
            rotation: self.state.rotation,
            clip: self.state.clip,
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
            rotation: self.state.rotation,
            clip: self.state.clip,
        });
    }

    pub fn line(&self, from: Pos2, to: Pos2, width: f32, color: Color32) {
        if color.alpha() == 0 || width <= 0.0 {
            return;
        }
        self.push(Shape::Line {
            from: self.state.rotation.apply(from),
            to: self.state.rotation.apply(to),
            width,
            color,
            clip: self.state.clip,
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
            rotation: self.state.rotation,
            clip: self.state.clip,
        });
    }

    pub fn punch(&self, rect: Rect, corner_radius: f32) {
        if !rect.is_positive() {
            return;
        }
        self.push(Shape::Punch {
            rect,
            corner_radius,
            rotation: self.state.rotation,
            clip: self.state.clip,
        });
    }

    pub fn drawing(&self, rect: Rect, drawing: &Drawing) {
        if !rect.is_positive() {
            return;
        }
        self.push(Shape::Drawing {
            rect,
            drawing: drawing.clone(),
            clip: self.state.clip,
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
            rotation: self.state.rotation,
            clip: self.state.clip,
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
