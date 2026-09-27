use std::collections::HashMap;
use std::time::Instant;

use crate::damage::Region;
use crate::geometry::{Rect, Vec2};
use crate::painter::{Painter, PainterState, Shape};

use crate::document::Document;
use crate::node::{ANCESTOR_LIMIT, Arena, NodeId, NodeMap, Rects};

pub(crate) type Entry = Option<(Vec2, Rect)>;

#[derive(Clone, PartialEq)]
pub(crate) enum Item {
    Main(Shape),
    Top(Shape),
    Child(NodeId, Entry),
}

#[derive(Clone, Copy)]
struct Placement {
    rect: Rect,
    painter: PainterState,
}

pub(crate) struct Recorded {
    pub(crate) items: Vec<Item>,
    pub(crate) own: Rect,
    pub(crate) bounds: Rect,
    pub(crate) deadline: Option<Instant>,
    pub(crate) reads: bool,
}

struct Painted {
    placement: Placement,
    items: Vec<Item>,
    own: Rect,
    bounds: Rect,
    reads: bool,
    below: bool,
}

#[derive(Clone, Copy, Default)]
struct Dirt {
    own: bool,
    below: bool,
    batch: u64,
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
    batch: u64,
}

impl PaintCache {
    fn bounds(&self, id: NodeId) -> Rect {
        self.entries
            .get(&id)
            .map_or(Rect::NOTHING, |painted| painted.bounds)
    }

    pub(crate) fn absolute_bounds(&self, id: NodeId) -> Rect {
        self.entries.get(&id).map_or(Rect::NOTHING, |painted| {
            absolute(painted.bounds, painted.placement.painter)
        })
    }

    pub(crate) fn forget(&mut self, id: NodeId) {
        self.entries.remove(&id);
        self.dirt.remove(&id);
        self.deadlines.remove(&id);
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn mark(&mut self, ids: &[NodeId], arena: &Arena) {
        self.batch = self.batch.wrapping_add(1);
        for id in ids {
            self.mark_one(*id, arena);
        }
    }

    fn mark_one(&mut self, id: NodeId, arena: &Arena) {
        if !arena.current(id) {
            return;
        }
        self.dirt.get_or_default(id).own = true;
        let mut current = arena.parent(id);
        for _ in 0..ANCESTOR_LIMIT {
            let Some(node) = current else {
                return;
            };
            let dirt = self.dirt.get_or_default(node);
            if dirt.below && dirt.batch == self.batch {
                return;
            }
            dirt.below = true;
            dirt.batch = self.batch;
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
        let roots: Vec<(NodeId, Rect)> = roots
            .into_iter()
            .map(|id| (id, self.absolute_bounds(id)))
            .collect();
        let held: Vec<NodeId> = self.roots.iter().map(|(id, _)| *id).collect();
        if roots.iter().map(|(id, _)| *id).ne(held) {
            let before = kept(&self.roots, &roots);
            let after = kept(&roots, &self.roots);
            let reordered = before
                .iter()
                .zip(&after)
                .position(|(before, after)| before.0 != after.0)
                .unwrap_or(before.len());
            let added = roots.iter().filter(|root| !contains(&self.roots, root.0));
            let removed = self.roots.iter().filter(|root| !contains(&roots, root.0));
            let restacked = before[reordered..].iter().chain(&after[reordered..]);
            for (_, bounds) in added.chain(removed).chain(restacked) {
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
            let origin = self
                .entries
                .get(root)
                .map_or(Vec2::ZERO, |painted| painted.placement.painter.origin);
            self.flatten_node(*root, origin, Rect::EVERYTHING, &mut main, &mut top);
            main.append(&mut top);
        }
        main
    }

    fn flatten_node(
        &self,
        id: NodeId,
        origin: Vec2,
        clip: Rect,
        main: &mut Vec<Shape>,
        top: &mut Vec<Shape>,
    ) {
        let Some(painted) = self.entries.get(&id) else {
            return;
        };
        let moved = origin != Vec2::ZERO || clip != Rect::EVERYTHING;
        let place = |shape: &Shape, into: &mut Vec<Shape>| match moved {
            false => into.push(shape.clone()),
            true => {
                let shape = placed_shape(shape, origin, clip);
                if crate::damage::bounds(&shape).is_positive()
                    || matches!(shape, Shape::Drawing { .. })
                {
                    into.push(shape);
                }
            }
        };
        for item in &painted.items {
            match item {
                Item::Main(shape) => place(shape, main),
                Item::Top(shape) => place(shape, top),
                Item::Child(child, None) => self.flatten_node(*child, origin, clip, main, top),
                Item::Child(child, Some((translation, kept))) => self.flatten_node(
                    *child,
                    origin + *translation,
                    clip.intersect(kept.translate(origin)),
                    main,
                    top,
                ),
            }
        }
    }

    fn visit(&mut self, id: NodeId, placement: Placement) -> Visit {
        let dirt = self.dirt.remove(&id).unwrap_or_default();
        let Some(painted) = self.entries.get_mut(&id).filter(|painted| {
            !dirt.own
                && painted.placement.rect == placement.rect
                && painted.placement.painter.settles(placement.painter)
        }) else {
            return Visit::Record;
        };
        let sees = painted.placement.painter.sees(placement.painter);
        if painted.reads && !sees {
            return Visit::Record;
        }
        painted.placement = placement;
        if !dirt.below && (sees || !painted.below) {
            return Visit::Reuse(painted.bounds);
        }
        let entered: Vec<(NodeId, Entry)> = children(&painted.items).collect();
        let mut children = Vec::new();
        for (child, entry) in entered {
            let Some(held) = self.entries.get(&child) else {
                return Visit::Record;
            };
            children.push((
                child,
                resumed(placement.painter, held.placement.painter, entry),
            ));
        }
        Visit::Descend(children)
    }

    fn settle_descended(&mut self, id: NodeId) -> Rect {
        let Some(painted) = self.entries.get(&id) else {
            return Rect::NOTHING;
        };
        let bounds = children(&painted.items)
            .map(|(child, entry)| entered_bounds(self.bounds(child), entry))
            .fold(painted.own, |bounds, child| bounds.union(child));
        let below = self.reads_below(&painted.items);
        if let Some(painted) = self.entries.get_mut(&id) {
            painted.bounds = bounds;
            painted.below = below;
        }
        bounds
    }

    fn reads_below(&self, items: &[Item]) -> bool {
        children(items).any(|(child, _)| {
            self.entries
                .get(&child)
                .is_some_and(|painted| painted.reads || painted.below)
        })
    }

    fn store(&mut self, id: NodeId, placement: Placement, recorded: Recorded) {
        let below = self.reads_below(&recorded.items);
        let painted = Painted {
            placement,
            items: recorded.items,
            own: recorded.own,
            bounds: recorded.bounds,
            reads: recorded.reads,
            below,
        };
        let state = placement.painter;
        let old = self.entries.remove(&id);
        let redrawn = old
            .as_ref()
            .and_then(|old| redrawn_damage(&old.items, &painted.items));
        let damaged: Vec<Rect> = match old {
            None => vec![painted.bounds],
            Some(_) if redrawn.is_some() => redrawn.unwrap_or_default().rects().to_vec(),
            Some(old) if children(&old.items).eq(children(&painted.items)) => {
                vec![old.own, painted.own]
            }
            Some(old) => vec![old.bounds, painted.bounds],
        };
        for rect in damaged {
            self.damage.add(absolute(rect, state));
        }
        match recorded.deadline {
            Some(deadline) => self.deadlines.insert(id, deadline),
            None => self.deadlines.remove(&id),
        };
        self.entries.insert(id, painted);
        self.recorded = true;
    }
}

fn resumed(parent: PainterState, held: PainterState, entry: Entry) -> PainterState {
    let (space_clip, origin) = match entry {
        Some((translation, kept)) => (
            parent.space_clip.intersect(kept).translate(-translation),
            parent.origin + translation,
        ),
        None => (parent.space_clip, parent.origin),
    };
    PainterState {
        space_clip,
        origin,
        entry,
        ..held
    }
}

fn absolute(rect: Rect, state: PainterState) -> Rect {
    rect.intersect(state.space_clip).translate(state.origin)
}

fn entered_bounds(bounds: Rect, entry: Entry) -> Rect {
    match entry {
        Some((translation, kept)) => bounds.translate(translation).intersect(kept),
        None => bounds,
    }
}

fn contains(roots: &[(NodeId, Rect)], id: NodeId) -> bool {
    roots.iter().any(|(root, _)| *root == id)
}

fn kept(roots: &[(NodeId, Rect)], others: &[(NodeId, Rect)]) -> Vec<(NodeId, Rect)> {
    roots
        .iter()
        .filter(|(id, _)| contains(others, *id))
        .copied()
        .collect()
}

fn redrawn_damage(old: &[Item], new: &[Item]) -> Option<Region> {
    if old.len() != new.len() {
        return None;
    }
    let mut region = Region::NOTHING;
    for pair in old.iter().zip(new) {
        match pair {
            (old, new) if old == new => {}
            (Item::Main(old), Item::Main(new)) | (Item::Top(old), Item::Top(new)) => {
                region = region.union(redrawn_in_place(old, new)?);
            }
            _ => return None,
        }
    }
    Some(region)
}

pub(crate) fn redrawn_in_place(old: &Shape, new: &Shape) -> Option<Region> {
    let (
        Shape::Drawing {
            rect,
            clip,
            drawing,
        },
        Shape::Drawing {
            rect: new_rect,
            clip: new_clip,
            drawing: new_drawing,
        },
    ) = (old, new)
    else {
        return None;
    };
    if rect != new_rect || clip != new_clip {
        return None;
    }
    let damage = new_drawing.damage_since(drawing)?;
    let visible = rect.intersect(*clip);
    let mut region = Region::NOTHING;
    for damaged in damage.rects() {
        region.add(damaged.translate(rect.min.to_vec2()).intersect(visible));
    }
    Some(region)
}

fn children(items: &[Item]) -> impl Iterator<Item = (NodeId, Entry)> + '_ {
    items.iter().filter_map(|item| match item {
        Item::Child(child, entry) => Some((*child, *entry)),
        Item::Main(_) | Item::Top(_) => None,
    })
}

pub(crate) fn paint(doc: &Document, painter: &Painter, rects: &Rects, id: NodeId) {
    let rect = rects.placed(&id).expect("node was not placed").rect;
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
            let outer = ctx.swap_space_read(false);
            doc.arena.get(id).paint(doc, &painter.inner(), rects, rect);
            let reads = ctx.swap_space_read(outer);
            let recorded = Recorded {
                reads,
                ..ctx.exit_paint()
            };
            let bounds = recorded.bounds;
            doc.paint_cache.borrow_mut().store(id, placement, recorded);
            bounds
        }
    };
    let entry = placement.painter.entry;
    ctx.paint_child(id, entry, entered_bounds(bounds, entry));
}

fn placed_shape(shape: &Shape, offset: Vec2, space_clip: Rect) -> Shape {
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
