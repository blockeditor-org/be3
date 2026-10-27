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
use crate::node::{NodeId, SpaceId, SpaceTransform};
use crate::path::{FillRule, Path, PathPaint, PathStyle, Stroke};
use crate::pixel_grid::PixelGrid;
use crate::rich::{Piece, RichLayout, RichOptions, Shaper};

pub use beui_types::layout::Corners;

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
        scale: f32,
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
    Path {
        path: Rc<PathPaint>,
        scale: f32,
        offset: Vec2,
        color: Color32,
        rotation: Rotation,
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
                    scale,
                    color,
                    rotation,
                    clip,
                },
                Self::Text {
                    origin: other_origin,
                    galley: other_galley,
                    scale: other_scale,
                    color: other_color,
                    rotation: other_rotation,
                    clip: other_clip,
                },
            ) => {
                origin == other_origin
                    && scale == other_scale
                    && color == other_color
                    && rotation == other_rotation
                    && clip == other_clip
                    && galley.same_layout(other_galley)
            }
            _ => self == other,
        }
    }
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

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Zoom {
    pub scale: f32,
    pub raster: f32,
}

impl Zoom {
    pub const NONE: Self = Self {
        scale: 1.0,
        raster: 1.0,
    };
}

static SCOPES: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PainterState {
    pub clip: Rect,
    pub space_clip: Rect,
    pub origin: Vec2,
    pub scale: f32,
    pub raster: f32,
    pub space: Option<SpaceId>,
    pub scroll: Option<SpaceId>,
    pub rotation: Rotation,
}

impl PainterState {
    pub fn settles(self, other: Self) -> bool {
        self.clip == other.clip && self.space == other.space && self.rotation == other.rotation
    }

    pub fn sees(self, other: Self) -> bool {
        self.space_clip == other.space_clip
            && self.origin == other.origin
            && self.scale == other.scale
    }

    pub fn to_screen(self, rect: Rect) -> Rect {
        SpaceTransform {
            offset: self.origin,
            scale: self.scale,
        }
        .rect(rect)
    }

    pub fn to_local(self, rect: Rect) -> Rect {
        SpaceTransform {
            offset: self.origin,
            scale: self.scale,
        }
        .local_rect(rect)
    }

    fn inside(self, clip: Rect, space_clip: Rect, offset: Vec2, scale: f32) -> Self {
        let local = |rect: Rect| match scale == 1.0 {
            true => rect.translate(-offset),
            false => rect.translate(-offset).scaled(scale.recip()),
        };
        let rotation = match scale == 1.0 {
            true => self.rotation.translate(-offset),
            false => self.rotation.translate(-offset).scaled(scale.recip()),
        };
        Self {
            clip: local(clip),
            space_clip: local(space_clip),
            origin: self.origin + offset * self.scale,
            scale: self.scale * scale,
            rotation,
            ..self
        }
    }

    pub fn within(self, group: &Group) -> Self {
        match *group {
            Group::Clip(clip) => Self {
                clip: self.clip.intersect(clip),
                ..self
            },
            Group::Scroll {
                space,
                viewport,
                offset,
                scale,
                raster,
            } => Self {
                space: Some(space),
                scroll: Some(space),
                raster: self.raster * raster,
                ..self.inside(
                    Rect::EVERYTHING,
                    self.space_clip.intersect(viewport),
                    offset,
                    scale,
                )
            },
            Group::Transform { offset, scale } => Self {
                space: None,
                ..self.inside(
                    self.clip,
                    self.space_clip.intersect(self.clip),
                    offset,
                    scale,
                )
            },
            Group::Mask { .. } | Group::Filter(_) => self,
        }
    }
}

thread_local! {
    static COLORS: std::cell::RefCell<Option<(Rc<crate::color::ColorDefinitions>, crate::color::ColorScheme)>> =
        const { std::cell::RefCell::new(None) };
}

pub(crate) fn with_colors<R>(
    definitions: Rc<crate::color::ColorDefinitions>,
    scheme: crate::color::ColorScheme,
    run: impl FnOnce() -> R,
) -> R {
    let held = COLORS.with(|colors| colors.replace(Some((definitions, scheme))));
    let result = run();
    COLORS.with(|colors| *colors.borrow_mut() = held);
    result
}

pub struct Painter {
    context: Context,
    state: PainterState,
    scope: Option<Rc<Scope>>,
}

impl Painter {
    pub fn resolve_color(&self, color: crate::color::Color) -> Color32 {
        if let Some(color) = color.as_literal() {
            return color;
        }
        COLORS.with(|colors| match &*colors.borrow() {
            Some((definitions, scheme)) => color.resolve_in(definitions, *scheme),
            None => color.resolve(|_| None),
        })
    }

    pub fn new(context: Context, clip: Rect) -> Self {
        Self {
            context,
            state: PainterState {
                clip,
                space_clip: Rect::EVERYTHING,
                origin: Vec2::ZERO,
                scale: 1.0,
                raster: 1.0,
                space: None,
                scroll: None,
                rotation: Rotation::NONE,
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

    fn grouped(&self, group: Group) -> Self {
        Self {
            context: self.context.clone(),
            state: self.state.within(&group),
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
                origin: state.origin + translation * state.scale,
                space: Some(SpaceId::of(node)),
                rotation: state.rotation.translate(-translation),
                ..state
            },
            scope: None,
        }
    }

    pub fn shifted(&self, space: SpaceId, by: Vec2, clip: Rect) -> Self {
        self.zoomed(space, by, Zoom::NONE, clip)
    }

    pub fn zoomed(&self, space: SpaceId, by: Vec2, zoom: Zoom, clip: Rect) -> Self {
        self.grouped(Group::Scroll {
            space,
            viewport: self.state.clip.intersect(self.state.rotation.bounds(clip)),
            offset: by,
            scale: zoom.scale,
            raster: zoom.raster,
        })
    }

    pub fn faded(&self, space: SpaceId, rect: Rect, widths: [f32; 4], opacity: f32) -> Self {
        self.grouped(Group::Mask {
            space,
            fade: Fade::faded(rect, widths, opacity),
        })
    }

    pub fn transformed(&self, offset: Vec2, scale: f32) -> Self {
        self.grouped(Group::Transform { offset, scale })
    }

    pub fn in_document(&self) -> Self {
        self.context.note_space_read();
        let scale = self.state.scale.recip();
        self.grouped(Group::Transform {
            offset: -self.state.origin * scale,
            scale,
        })
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

    pub fn to_screen(&self, rect: Rect) -> Rect {
        self.context.note_space_read();
        self.state.to_screen(rect)
    }

    pub fn to_local(&self, rect: Rect) -> Rect {
        self.context.note_space_read();
        self.state.to_local(rect)
    }

    pub fn pixel_grid(&self) -> PixelGrid {
        PixelGrid::new(self.context.pixels_per_point())
    }

    pub fn with_clip_rect(&self, clip: Rect) -> Self {
        match clip == Rect::EVERYTHING {
            true => self.with(self.state),
            false => self.grouped(Group::Clip(self.state.rotation.bounds(clip))),
        }
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
        self.context.paint_child(&self.scope, id, at);
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

    pub fn rich_layout(&self, pieces: &[Piece], options: RichOptions) -> RichLayout {
        RichLayout::new(pieces, options, &mut PainterShaper(self))
    }

    pub fn rect_filled(&self, rect: Rect, corner_radius: impl Into<Corners>, color: Color32) {
        if color.alpha() == 0 {
            return;
        }
        self.push(Shape::Rect {
            rect,
            corner_radius: corner_radius.into().fitted(rect),
            stroke_width: 0.0,
            color,
            rotation: self.state.rotation,
            clip: Rect::EVERYTHING,
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
            corner_radius: corner_radius.into().fitted(rect.expand(width)),
            stroke_width: width,
            color,
            rotation: self.state.rotation,
            clip: Rect::EVERYTHING,
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
            clip: Rect::EVERYTHING,
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
            clip: Rect::EVERYTHING,
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
            clip: Rect::EVERYTHING,
        });
    }

    pub fn drawing(&self, rect: Rect, drawing: &Drawing) {
        if !rect.is_positive() {
            return;
        }
        self.push(Shape::Drawing {
            rect,
            drawing: drawing.clone(),
            clip: Rect::EVERYTHING,
        });
    }

    pub fn fill_path(&self, path: &Path, rule: FillRule, color: Color32) {
        self.path(path, PathStyle::Fill(rule), color);
    }

    pub fn stroke_path(&self, path: &Path, stroke: &Stroke, color: Color32) {
        if stroke.width.is_nan() || stroke.width <= 0.0 {
            return;
        }
        self.path(path, PathStyle::Stroke(stroke.clone()), color);
    }

    fn path(&self, path: &Path, style: PathStyle, color: Color32) {
        if color.alpha() == 0 || path.is_empty() {
            return;
        }
        self.paint_path(&Rc::new(PathPaint::new(path.clone(), style)), color);
    }

    pub fn paint_path(&self, path: &Rc<PathPaint>, color: Color32) {
        if color.alpha() == 0 || path.path().is_empty() {
            return;
        }
        self.push(Shape::Path {
            path: Rc::clone(path),
            scale: 1.0,
            offset: Vec2::ZERO,
            color,
            rotation: self.state.rotation,
            clip: Rect::EVERYTHING,
        });
    }

    pub fn galley(&self, origin: Pos2, galley: Galley, color: Color32) {
        if color.alpha() == 0 || galley.is_blank() {
            return;
        }
        let galley = match self.state.raster == 1.0 {
            true => galley,
            false => self.context.rastered(&galley, self.state.raster),
        };
        self.push(Shape::Text {
            origin,
            galley,
            scale: 1.0,
            color,
            rotation: self.state.rotation,
            clip: Rect::EVERYTHING,
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
        Shape::Path {
            offset: at,
            rotation,
            clip,
            ..
        } => {
            *at += offset;
            *rotation = rotation.translate(offset);
            *clip = clip.translate(offset).intersect(space_clip);
        }
    }
    shape
}

struct PainterShaper<'a>(&'a Painter);

impl Shaper for PainterShaper<'_> {
    fn galley(&mut self, text: &str, font: FontId) -> Galley {
        self.0.layout_text(text, font, TextLayout::DEFAULT)
    }

    fn slice(&mut self, galley: &Galley, range: std::ops::Range<usize>, _font: FontId) -> Galley {
        galley.slice(range, self.0.ctx().pixels_per_point())
    }
}
