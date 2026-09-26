use std::collections::HashMap;
use std::time::Instant;

use crate::damage::Region;
use crate::geometry::Rect;
use crate::painter::{Painter, PainterState, Shape};

use crate::document::Document;
use crate::node::{ANCESTOR_LIMIT, Arena, NodeId, NodeMap};

#[derive(Clone, PartialEq)]
pub(crate) enum Item {
    Main(Shape),
    Top(Shape),
    Child(NodeId),
}

#[derive(Clone, Copy, PartialEq)]
struct Placement {
    rect: Rect,
    painter: PainterState,
}

pub(crate) struct Recorded {
    pub(crate) items: Vec<Item>,
    pub(crate) own: Rect,
    pub(crate) bounds: Rect,
    pub(crate) deadline: Option<Instant>,
}

struct Painted {
    placement: Placement,
    items: Vec<Item>,
    own: Rect,
    bounds: Rect,
}

#[derive(Clone, Copy, Default)]
struct Dirt {
    own: bool,
    below: bool,
}

enum Visit {
    Reuse(Rect),
    Descend(Vec<(NodeId, PainterState)>),
    Record,
}

#[derive(Default)]
pub(crate) struct PaintCache {
    entries: NodeMap<Painted>,
    dirt: NodeMap<Dirt>,
    deadlines: HashMap<NodeId, Instant>,
    roots: Vec<(NodeId, Rect)>,
    damage: Region,
    recorded: bool,
}

impl PaintCache {
    pub(crate) fn bounds(&self, id: NodeId) -> Rect {
        self.entries
            .get(&id)
            .map_or(Rect::NOTHING, |painted| painted.bounds)
    }

    pub(crate) fn forget(&mut self, id: NodeId) {
        self.entries.remove(&id);
        self.dirt.remove(&id);
        self.deadlines.remove(&id);
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn mark(&mut self, id: NodeId, arena: &Arena) {
        self.dirt.get_or_default(id).own = true;
        let mut current = arena.parent(id);
        for _ in 0..ANCESTOR_LIMIT {
            let Some(node) = current else {
                return;
            };
            self.dirt.get_or_default(node).below = true;
            current = arena.parent(node);
        }
    }

    pub(crate) fn take_due(&mut self, now: Instant) -> Vec<NodeId> {
        let due: Vec<NodeId> = self
            .deadlines
            .iter()
            .filter(|(_, deadline)| **deadline <= now)
            .map(|(id, _)| *id)
            .collect();
        for id in &due {
            self.deadlines.remove(id);
        }
        due
    }

    #[cfg(test)]
    pub(crate) fn expire_deadlines(&mut self, now: Instant) {
        for deadline in self.deadlines.values_mut() {
            *deadline = now;
        }
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.deadlines.values().min().copied()
    }

    pub(crate) fn settle_roots(&mut self, roots: Vec<NodeId>) {
        let roots: Vec<(NodeId, Rect)> = roots.into_iter().map(|id| (id, self.bounds(id))).collect();
        let held: Vec<NodeId> = self.roots.iter().map(|(id, _)| *id).collect();
        if roots.iter().map(|(id, _)| *id).ne(held) {
            for (_, bounds) in self.roots.iter().chain(&roots) {
                self.damage.add(*bounds);
            }
            self.recorded = true;
        }
        self.roots = roots;
    }

    pub(crate) fn take_damage(&mut self) -> Region {
        std::mem::take(&mut self.damage)
    }

    pub(crate) fn take_recorded(&mut self) -> bool {
        std::mem::take(&mut self.recorded)
    }

    pub(crate) fn flatten(&self) -> Vec<Shape> {
        let mut main = Vec::new();
        let mut top = Vec::new();
        for (root, _) in &self.roots {
            self.flatten_node(*root, &mut main, &mut top);
            main.append(&mut top);
        }
        main
    }

    fn flatten_node(&self, id: NodeId, main: &mut Vec<Shape>, top: &mut Vec<Shape>) {
        let Some(painted) = self.entries.get(&id) else {
            return;
        };
        for item in &painted.items {
            match item {
                Item::Main(shape) => main.push(shape.clone()),
                Item::Top(shape) => top.push(shape.clone()),
                Item::Child(child) => self.flatten_node(*child, main, top),
            }
        }
    }

    fn visit(&mut self, id: NodeId, placement: Placement) -> Visit {
        let dirt = self.dirt.remove(&id).unwrap_or_default();
        let Some(painted) = self
            .entries
            .get(&id)
            .filter(|painted| !dirt.own && painted.placement == placement)
        else {
            return Visit::Record;
        };
        if !dirt.below {
            return Visit::Reuse(painted.bounds);
        }
        let mut children = Vec::new();
        for item in &painted.items {
            if let Item::Child(child) = item {
                let Some(held) = self.entries.get(child) else {
                    return Visit::Record;
                };
                children.push((*child, held.placement.painter));
            }
        }
        Visit::Descend(children)
    }

    fn settle_descended(&mut self, id: NodeId) -> Rect {
        let Some(painted) = self.entries.get(&id) else {
            return Rect::NOTHING;
        };
        let bounds = painted
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Child(child) => Some(self.bounds(*child)),
                Item::Main(_) | Item::Top(_) => None,
            })
            .fold(painted.own, |bounds, child| bounds.union(child));
        if let Some(painted) = self.entries.get_mut(&id) {
            painted.bounds = bounds;
        }
        bounds
    }

    fn store(&mut self, id: NodeId, placement: Placement, recorded: Recorded) {
        let painted = Painted {
            placement,
            items: recorded.items,
            own: recorded.own,
            bounds: recorded.bounds,
        };
        match self.entries.remove(&id) {
            None => self.damage.add(painted.bounds),
            Some(old) if old.items == painted.items => {}
            Some(old) if children(&old.items).eq(children(&painted.items)) => {
                self.damage.add(old.own);
                self.damage.add(painted.own);
            }
            Some(old) => {
                self.damage.add(old.bounds);
                self.damage.add(painted.bounds);
            }
        }
        match recorded.deadline {
            Some(deadline) => self.deadlines.insert(id, deadline),
            None => self.deadlines.remove(&id),
        };
        self.entries.insert(id, painted);
        self.recorded = true;
    }
}

fn children(items: &[Item]) -> impl Iterator<Item = NodeId> + '_ {
    items.iter().filter_map(|item| match item {
        Item::Child(child) => Some(*child),
        Item::Main(_) | Item::Top(_) => None,
    })
}

pub(crate) fn paint(doc: &Document, painter: &Painter, rects: &NodeMap<Rect>, id: NodeId) {
    let rect = rects[&id];
    let ctx = painter.ctx();
    let placement = Placement {
        rect,
        painter: painter.state(),
    };
    let visit = doc.paint_cache.borrow_mut().visit(id, placement);
    let bounds = match visit {
        Visit::Reuse(bounds) => {
            doc.note_painted(true);
            bounds
        }
        Visit::Descend(children) => {
            doc.note_painted(true);
            ctx.measure_paint(|| {
                for (child, state) in children {
                    if rects.contains_key(&child) {
                        paint(doc, &Painter::resumed(ctx.clone(), state), rects, child);
                    }
                }
            });
            doc.paint_cache.borrow_mut().settle_descended(id)
        }
        Visit::Record => {
            doc.note_painted(false);
            ctx.enter_paint(id);
            doc.arena.get(id).paint(doc, painter, rects, rect);
            let recorded = ctx.exit_paint();
            let bounds = recorded.bounds;
            doc.paint_cache.borrow_mut().store(id, placement, recorded);
            bounds
        }
    };
    ctx.paint_child(id, bounds);
}

#[cfg(test)]
pub(crate) fn same_shape(left: &Shape, right: &Shape) -> bool {
    match (left, right) {
        (
            Shape::Text {
                origin,
                galley,
                color,
                rotation,
                clip,
            },
            Shape::Text {
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
                && galley.size() == other_galley.size()
                && galley.glyphs().len() == other_galley.glyphs().len()
        }
        (
            Shape::Drawing { rect, clip, .. },
            Shape::Drawing {
                rect: other_rect,
                clip: other_clip,
                ..
            },
        ) => rect == other_rect && clip == other_clip,
        _ => left == right,
    }
}
