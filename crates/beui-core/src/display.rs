use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::context::scale_shape;
use crate::fade::Fade;
use crate::filter::Filter;
use crate::node::SpaceId;
use crate::painter::{Shape, placed_shape};
use beui_types::NodeId;
use beui_types::{Rect, Vec2};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Group {
    Transform {
        offset: Vec2,
        scale: f32,
    },
    Clip(Rect),
    Scroll {
        space: SpaceId,
        viewport: Rect,
        offset: Vec2,
        scale: f32,
        raster: f32,
    },
    Mask {
        space: SpaceId,
        fade: Fade,
    },
    Filter(Filter),
}

impl Group {
    pub fn place(&self, rect: Rect) -> Rect {
        match *self {
            Self::Transform { offset, scale } => rect.scaled(scale).translate(offset),
            Self::Clip(clip) => rect.intersect(clip),
            Self::Scroll {
                viewport,
                offset,
                scale,
                ..
            } => rect.scaled(scale).translate(offset).intersect(viewport),
            Self::Mask { .. } => rect,
            Self::Filter(filter) => match filter.blur > 0.0 && rect.is_positive() {
                true => rect.union(rect.expand(filter.blur * 3.0).intersect(filter.region)),
                false => rect,
            },
        }
    }

    pub fn same_place(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Scroll {
                    space, viewport, ..
                },
                Self::Scroll {
                    space: other_space,
                    viewport: other_viewport,
                    ..
                },
            ) => space == other_space && viewport == other_viewport,
            (Self::Mask { space, .. }, Self::Mask { space: other, .. }) => space == other,
            _ => self == other,
        }
    }

    pub fn place_shape(&self, shape: &mut Shape) {
        match *self {
            Self::Transform { offset, scale } => {
                if scale != 1.0 {
                    scale_shape(shape, scale);
                }
                *shape = placed_shape(shape, offset, Rect::EVERYTHING);
            }
            Self::Clip(clip) => {
                let held = crate::context::shape_clip(shape);
                *held = held.intersect(clip);
            }
            Self::Scroll {
                viewport,
                offset,
                scale,
                ..
            } => {
                if scale != 1.0 {
                    scale_shape(shape, scale);
                }
                *shape = placed_shape(shape, offset, viewport);
            }
            Self::Mask { .. } | Self::Filter(_) => {}
        }
    }

    pub fn place_fade(&self, fade: Fade) -> Fade {
        match *self {
            Self::Transform { offset, scale } | Self::Scroll { offset, scale, .. } => {
                fade.scaled(scale, offset)
            }
            Self::Mask { fade: mask, .. } => mask.within(fade),
            Self::Clip(_) | Self::Filter(_) => fade,
        }
    }
}

pub struct Display {
    pub document: u64,
    pub node: NodeId,
    pub key: u64,
    pub shapes: Rc<[Shape]>,
    pub parts: Box<[Part]>,
    pub bounds: Rect,
    pub count: usize,
}

pub enum Part {
    Shapes { start: u32, end: u32, bounds: Rect },
    Child(NodeId, Vec2, Rc<Display>),
    Group(Group, Box<[Part]>),
}

impl Part {
    pub fn bounds(&self) -> Rect {
        match self {
            Self::Shapes { bounds, .. } => *bounds,
            Self::Child(_, at, display) => display.bounds.translate(*at),
            Self::Group(group, parts) => group.place(bounds_of(parts)),
        }
    }
}

pub fn bounds_of(parts: &[Part]) -> Rect {
    parts
        .iter()
        .fold(Rect::NOTHING, |bounds, part| bounds.union(part.bounds()))
}

fn count_of(parts: &[Part]) -> usize {
    parts
        .iter()
        .map(|part| match part {
            Part::Shapes { start, end, .. } => (end - start) as usize,
            Part::Child(_, _, display) => display.count,
            Part::Group(_, parts) => count_of(parts),
        })
        .sum()
}

static KEYS: AtomicU64 = AtomicU64::new(1);

impl Display {
    pub fn new(document: u64, node: NodeId, shapes: Vec<Shape>, parts: Vec<Part>) -> Self {
        Self {
            document,
            node,
            key: KEYS.fetch_add(1, Ordering::Relaxed),
            shapes: shapes.into(),
            bounds: bounds_of(&parts),
            count: count_of(&parts),
            parts: parts.into_boxed_slice(),
        }
    }

    pub fn with_children(&self, child: &impl Fn(NodeId) -> Option<Rc<Display>>) -> Option<Self> {
        let parts = rechilded(&self.parts, child)?;
        Some(Self {
            document: self.document,
            node: self.node,
            key: self.key,
            shapes: Rc::clone(&self.shapes),
            bounds: bounds_of(&parts),
            count: count_of(&parts),
            parts,
        })
    }

    pub fn children(&self) -> Vec<(NodeId, Vec2, &Rc<Display>)> {
        let mut children = Vec::new();
        collect_children(&self.parts, &mut children);
        children
    }

    pub fn flatten(&self, at: Vec2, into: &mut Vec<Shape>) {
        let mut faded = Vec::new();
        self.flatten_faded(at, &mut faded);
        into.extend(faded.into_iter().map(|(shape, _)| shape));
    }

    pub fn flatten_faded(&self, at: Vec2, into: &mut Vec<(Shape, Fade)>) {
        self.flatten_within(at, Rect::EVERYTHING, Fade::NONE, into);
    }

    fn flatten_within(&self, at: Vec2, clip: Rect, fade: Fade, into: &mut Vec<(Shape, Fade)>) {
        self.flatten_parts(&self.parts, at, clip, fade, into);
    }

    fn flatten_parts(
        &self,
        parts: &[Part],
        at: Vec2,
        clip: Rect,
        fade: Fade,
        into: &mut Vec<(Shape, Fade)>,
    ) {
        for part in parts {
            match part {
                Part::Shapes { start, end, .. } => {
                    for shape in &self.shapes[*start as usize..*end as usize] {
                        push_shown(into, placed_shape(shape, at, clip), fade);
                    }
                }
                Part::Child(_, translation, display) => {
                    display.flatten_within(at + *translation, clip, fade, into);
                }
                Part::Group(group, inner) => match *group {
                    Group::Clip(rect) => self.flatten_parts(
                        inner,
                        at,
                        clip.intersect(rect.translate(at)),
                        fade,
                        into,
                    ),
                    Group::Scroll {
                        viewport,
                        offset,
                        scale: 1.0,
                        ..
                    } => self.flatten_parts(
                        inner,
                        at + offset,
                        clip.intersect(viewport.translate(at)),
                        fade,
                        into,
                    ),
                    Group::Scroll {
                        viewport,
                        offset,
                        scale,
                        ..
                    } => {
                        let clip = clip.intersect(viewport.translate(at));
                        self.flatten_scaled(inner, at + offset, scale, clip, fade, into);
                    }
                    Group::Transform { offset, scale: 1.0 } => {
                        self.flatten_parts(inner, at + offset, clip, fade, into)
                    }
                    Group::Transform { offset, scale } => {
                        self.flatten_scaled(inner, at + offset, scale, clip, fade, into);
                    }
                    Group::Mask { fade: mask, .. } => {
                        self.flatten_parts(inner, at, clip, fade.within(mask.translate(at)), into)
                    }
                    Group::Filter(_) => self.flatten_parts(inner, at, clip, fade, into),
                },
            }
        }
    }

    fn flatten_scaled(
        &self,
        parts: &[Part],
        at: Vec2,
        scale: f32,
        clip: Rect,
        fade: Fade,
        into: &mut Vec<(Shape, Fade)>,
    ) {
        let mut scaled = Vec::new();
        self.flatten_parts(parts, Vec2::ZERO, Rect::EVERYTHING, Fade::NONE, &mut scaled);
        for (mut shape, inner) in scaled {
            scale_shape(&mut shape, scale);
            push_shown(
                into,
                placed_shape(&shape, at, clip),
                fade.within(inner.scaled(scale, at)),
            );
        }
    }

    pub fn same_tree(&self, other: &Self) -> bool {
        same_parts(self, &self.parts, other, &other.parts)
    }
}

fn push_shown(into: &mut Vec<(Shape, Fade)>, shape: Shape, fade: Fade) {
    if crate::damage::bounds(&shape).is_positive() || matches!(shape, Shape::Drawing { .. }) {
        into.push((shape, fade));
    }
}

fn same_parts(left: &Display, parts: &[Part], right: &Display, others: &[Part]) -> bool {
    parts.len() == others.len()
        && parts.iter().zip(others).all(|pair| match pair {
            (
                Part::Shapes { start, end, .. },
                Part::Shapes {
                    start: other_start,
                    end: other_end,
                    ..
                },
            ) => {
                let shapes = &left.shapes[*start as usize..*end as usize];
                let other = &right.shapes[*other_start as usize..*other_end as usize];
                shapes.len() == other.len()
                    && shapes
                        .iter()
                        .zip(other)
                        .all(|(shape, other)| shape.same_look(other))
            }
            (Part::Child(id, at, display), Part::Child(other_id, other_at, other)) => {
                id == other_id && at == other_at && display.same_tree(other)
            }
            (Part::Group(group, inner), Part::Group(other_group, other_inner)) => {
                group == other_group && same_parts(left, inner, right, other_inner)
            }
            _ => false,
        })
}

fn rechilded(
    parts: &[Part],
    child: &impl Fn(NodeId) -> Option<Rc<Display>>,
) -> Option<Box<[Part]>> {
    let mut changed = false;
    let mut rebuilt = Vec::with_capacity(parts.len());
    for part in parts {
        rebuilt.push(match part {
            Part::Shapes { start, end, bounds } => Part::Shapes {
                start: *start,
                end: *end,
                bounds: *bounds,
            },
            Part::Child(id, at, display) => {
                let current = child(*id).unwrap_or_else(|| Rc::clone(display));
                changed |= !Rc::ptr_eq(&current, display);
                Part::Child(*id, *at, current)
            }
            Part::Group(group, inner) => match rechilded(inner, child) {
                Some(inner) => {
                    changed = true;
                    Part::Group(*group, inner)
                }
                None => Part::Group(*group, copied(inner)),
            },
        });
    }
    changed.then(|| rebuilt.into_boxed_slice())
}

fn copied(parts: &[Part]) -> Box<[Part]> {
    parts
        .iter()
        .map(|part| match part {
            Part::Shapes { start, end, bounds } => Part::Shapes {
                start: *start,
                end: *end,
                bounds: *bounds,
            },
            Part::Child(id, at, display) => Part::Child(*id, *at, Rc::clone(display)),
            Part::Group(group, inner) => Part::Group(*group, copied(inner)),
        })
        .collect()
}

fn collect_children<'a>(parts: &'a [Part], into: &mut Vec<(NodeId, Vec2, &'a Rc<Display>)>) {
    for part in parts {
        match part {
            Part::Shapes { .. } => {}
            Part::Child(id, at, display) => into.push((*id, *at, display)),
            Part::Group(_, inner) => collect_children(inner, into),
        }
    }
}

#[derive(Clone)]
pub enum Layer {
    Shape(Shape),
    Display(Vec2, Rc<Display>),
    Group(Group, Rc<[Layer]>),
}

impl Layer {
    pub fn bounds(&self) -> Rect {
        match self {
            Self::Shape(shape) => crate::damage::bounds(shape),
            Self::Display(at, display) => display.bounds.translate(*at),
            Self::Group(group, layers) => group.place(
                layers
                    .iter()
                    .fold(Rect::NOTHING, |bounds, layer| bounds.union(layer.bounds())),
            ),
        }
    }

    pub fn flatten(&self, into: &mut Vec<Shape>) {
        let mut faded = Vec::new();
        self.flatten_faded(&mut faded);
        into.extend(faded.into_iter().map(|(shape, _)| shape));
    }

    pub fn flatten_faded(&self, into: &mut Vec<(Shape, Fade)>) {
        match self {
            Self::Shape(shape) => into.push((shape.clone(), Fade::NONE)),
            Self::Display(at, display) => display.flatten_faded(*at, into),
            Self::Group(group, layers) => {
                let mut inner = Vec::new();
                for layer in layers.iter() {
                    layer.flatten_faded(&mut inner);
                }
                for (mut shape, fade) in inner {
                    group.place_shape(&mut shape);
                    push_shown(into, shape, group.place_fade(fade));
                }
            }
        }
    }

    pub fn filters(&self, into: &mut Vec<Filter>) {
        if let Self::Group(group, layers) = self {
            if let Group::Filter(filter) = group {
                into.push(*filter);
            }
            for layer in layers.iter() {
                layer.filters(into);
            }
        }
    }
}

pub fn flatten(layers: &[Layer]) -> Vec<Shape> {
    flatten_faded(layers)
        .into_iter()
        .map(|(shape, _)| shape)
        .collect()
}

pub fn flatten_faded(layers: &[Layer]) -> Vec<(Shape, Fade)> {
    let mut shapes = Vec::new();
    for layer in layers {
        layer.flatten_faded(&mut shapes);
    }
    shapes
}

pub fn filters(layers: &[Layer]) -> Vec<Filter> {
    let mut filters = Vec::new();
    for layer in layers {
        layer.filters(&mut filters);
    }
    filters
}
