use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::geometry::Rect;
use crate::node::NodeId;
use crate::painter::{Entry, Shape, placed_shape};

pub(crate) struct Display {
    pub(crate) key: u64,
    pub(crate) shapes: Rc<[Shape]>,
    pub(crate) parts: Box<[Part]>,
    pub(crate) bounds: Rect,
    pub(crate) count: usize,
}

pub(crate) enum Part {
    Shapes { top: bool, start: u32, end: u32 },
    Child(NodeId, Entry, Rc<Display>),
}

#[derive(PartialEq)]
pub(crate) enum Item<'a> {
    Main(&'a Shape),
    Top(&'a Shape),
    Child(NodeId, Entry),
}

static KEYS: AtomicU64 = AtomicU64::new(1);

impl Display {
    pub(crate) fn new(shapes: Vec<Shape>, parts: Vec<Part>, bounds: Rect) -> Self {
        let own = shapes.len();
        Self {
            key: KEYS.fetch_add(1, Ordering::Relaxed),
            shapes: shapes.into(),
            count: own + children_count(&parts),
            parts: parts.into_boxed_slice(),
            bounds,
        }
    }

    pub(crate) fn with_children(&self, parts: Vec<Part>, bounds: Rect) -> Self {
        Self {
            key: self.key,
            shapes: Rc::clone(&self.shapes),
            count: self.shapes.len() + children_count(&parts),
            parts: parts.into_boxed_slice(),
            bounds,
        }
    }

    pub(crate) fn items(&self) -> Vec<Item<'_>> {
        let mut items = Vec::new();
        for part in self.parts.iter() {
            match part {
                Part::Shapes { top, start, end } => {
                    for shape in &self.shapes[*start as usize..*end as usize] {
                        items.push(match top {
                            true => Item::Top(shape),
                            false => Item::Main(shape),
                        });
                    }
                }
                Part::Child(id, entry, _) => items.push(Item::Child(*id, *entry)),
            }
        }
        items
    }

    pub(crate) fn children(&self) -> impl Iterator<Item = (NodeId, Entry, &Rc<Display>)> + '_ {
        self.parts.iter().filter_map(|part| match part {
            Part::Child(id, entry, display) => Some((*id, *entry, display)),
            Part::Shapes { .. } => None,
        })
    }

    pub(crate) fn flatten(&self, at: Entry, main: &mut Vec<Shape>, top: &mut Vec<Shape>) {
        for part in self.parts.iter() {
            match part {
                Part::Shapes { top: on_top, start, end } => {
                    let into = match on_top {
                        true => &mut *top,
                        false => &mut *main,
                    };
                    for shape in &self.shapes[*start as usize..*end as usize] {
                        let shape = placed_shape(shape, at.translation, at.clip);
                        if crate::damage::bounds(&shape).is_positive()
                            || matches!(shape, Shape::Drawing { .. })
                        {
                            into.push(shape);
                        }
                    }
                }
                Part::Child(_, entry, display) => display.flatten(at.compose(*entry), main, top),
            }
        }
    }
}

fn children_count(parts: &[Part]) -> usize {
    parts
        .iter()
        .map(|part| match part {
            Part::Child(_, _, display) => display.count,
            Part::Shapes { .. } => 0,
        })
        .sum()
}

pub(crate) fn parts(items: Vec<crate::paint::Item>, child: impl Fn(NodeId) -> Option<Rc<Display>>) -> (Vec<Shape>, Vec<Part>) {
    let mut shapes = Vec::new();
    let mut parts: Vec<Part> = Vec::new();
    for item in items {
        let (shape, on_top) = match item {
            crate::paint::Item::Main(shape) => (shape, false),
            crate::paint::Item::Top(shape) => (shape, true),
            crate::paint::Item::Child(id, entry) => {
                if let Some(display) = child(id) {
                    parts.push(Part::Child(id, entry, display));
                }
                continue;
            }
        };
        let at = shapes.len() as u32;
        shapes.push(shape);
        match parts.last_mut() {
            Some(Part::Shapes { top, end, .. }) if *top == on_top && *end == at => *end = at + 1,
            _ => parts.push(Part::Shapes {
                top: on_top,
                start: at,
                end: at + 1,
            }),
        }
    }
    (shapes, parts)
}

#[derive(Clone)]
pub(crate) enum Layer {
    Shape(Shape),
    Display {
        display: Rc<Display>,
        entry: Entry,
        scale: f32,
        clip: Rect,
    },
}

impl Layer {
    pub(crate) fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Shape(left), Self::Shape(right)) => left == right,
            (
                Self::Display {
                    display,
                    entry,
                    scale,
                    clip,
                },
                Self::Display {
                    display: other_display,
                    entry: other_entry,
                    scale: other_scale,
                    clip: other_clip,
                },
            ) => {
                Rc::ptr_eq(display, other_display)
                    && entry == other_entry
                    && scale == other_scale
                    && clip == other_clip
            }
            _ => false,
        }
    }

    pub(crate) fn flatten(&self, into: &mut Vec<Shape>) {
        match self {
            Self::Shape(shape) => into.push(shape.clone()),
            Self::Display {
                display,
                entry,
                scale,
                clip,
            } => {
                let mut main = Vec::new();
                let mut top = Vec::new();
                display.flatten(*entry, &mut main, &mut top);
                for mut shape in main.into_iter().chain(top) {
                    crate::context::scale_shape(&mut shape, *scale);
                    let bounds = crate::context::shape_clip(&mut shape);
                    *bounds = bounds.intersect(*clip);
                    into.push(shape);
                }
            }
        }
    }
}

pub(crate) fn flatten(layers: &[Layer]) -> Vec<Shape> {
    let mut shapes = Vec::new();
    for layer in layers {
        layer.flatten(&mut shapes);
    }
    shapes
}
