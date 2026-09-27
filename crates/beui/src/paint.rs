use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use crate::damage::Region;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::display::{Display, Item as DisplayItem, Part};
use crate::painter::{Entry, Painter, PainterState, Shape};

use crate::document::Document;
use crate::node::{ANCESTOR_LIMIT, Arena, NodeId, NodeMap, Rects};

#[derive(Clone, PartialEq)]
pub(crate) enum Item {
    Main(Shape),
    Top(Shape),
    Child(NodeId, Entry),
}

#[derive(Clone, Copy)]
struct Placement {
    size: Vec2,
    given: PainterState,
    own: PainterState,
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
    entry: Entry,
    display: Rc<Display>,
    own: Rect,
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
    fn display(&self, id: NodeId) -> Option<Rc<Display>> {
        self.entries
            .get(&id)
            .map(|painted| Rc::clone(&painted.display))
    }

    pub(crate) fn absolute_bounds(&self, id: NodeId) -> Rect {
        self.entries.get(&id).map_or(Rect::NOTHING, |painted| {
            absolute(painted.display.bounds, painted.placement.own)
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
        for (id, bounds) in &roots {
            if let Some((_, held)) = self.roots.iter().find(|(root, _)| root == id)
                && held != bounds
            {
                self.damage.add(*held);
                self.damage.add(*bounds);
                self.recorded = true;
            }
        }
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

    pub(crate) fn painting(&self) -> Vec<(Rc<Display>, Entry)> {
        self.roots
            .iter()
            .filter_map(|(root, _)| {
                let painted = self.entries.get(root)?;
                Some((Rc::clone(&painted.display), painted.entry))
            })
            .collect()
    }

    fn visit(&mut self, id: NodeId, placement: Placement, entry: Entry) -> Visit {
        let dirt = self.dirt.remove(&id).unwrap_or_default();
        let Some(painted) = self.entries.get_mut(&id).filter(|painted| {
            !dirt.own
                && painted.placement.size == placement.size
                && painted.placement.own.settles(placement.own)
        }) else {
            return Visit::Record;
        };
        let sees = painted.placement.own.sees(placement.own);
        if painted.reads && !sees {
            return Visit::Record;
        }
        painted.placement = placement;
        painted.entry = entry;
        if !dirt.below && (sees || !painted.below) {
            return Visit::Reuse(painted.display.bounds);
        }
        let entered: Vec<NodeId> = painted.display.children().map(|(child, ..)| child).collect();
        let mut children = Vec::new();
        for child in entered {
            let Some(held) = self.entries.get(&child) else {
                return Visit::Record;
            };
            children.push((child, held.placement.given.resumed(placement.own)));
        }
        Visit::Descend(children)
    }

    fn settle_descended(&mut self, id: NodeId) -> Rect {
        let Some(painted) = self.entries.get(&id) else {
            return Rect::NOTHING;
        };
        let held = Rc::clone(&painted.display);
        let own = painted.own;
        let mut changed = false;
        let mut bounds = own;
        let mut parts = Vec::with_capacity(held.parts.len());
        for part in held.parts.iter() {
            parts.push(match part {
                Part::Shapes { top, start, end } => Part::Shapes {
                    top: *top,
                    start: *start,
                    end: *end,
                },
                Part::Child(child, entry, display) => {
                    let current = self.display(*child).unwrap_or_else(|| Rc::clone(display));
                    changed |= !Rc::ptr_eq(&current, display);
                    bounds = bounds.union(entry.place(current.bounds));
                    Part::Child(*child, *entry, current)
                }
            });
        }
        let below = self.reads_below(&held);
        if let Some(painted) = self.entries.get_mut(&id) {
            if changed || bounds != held.bounds {
                painted.display = Rc::new(held.with_children(parts, bounds));
                self.recorded = true;
            }
            painted.below = below;
        }
        bounds
    }

    fn reads_below(&self, display: &Display) -> bool {
        display.children().any(|(child, ..)| {
            self.entries
                .get(&child)
                .is_some_and(|painted| painted.reads || painted.below)
        })
    }

    fn store(&mut self, id: NodeId, placement: Placement, entry: Entry, recorded: Recorded) {
        let (shapes, parts) = crate::display::parts(recorded.items, |child| self.display(child));
        let display = Rc::new(Display::new(shapes, parts, recorded.bounds));
        let below = self.reads_below(&display);
        let painted = Painted {
            placement,
            entry,
            display,
            own: recorded.own,
            reads: recorded.reads,
            below,
        };
        let state = placement.own;
        let old = self.entries.remove(&id);
        let redrawn = old
            .as_ref()
            .and_then(|old| redrawn_damage(&old.display.items(), &painted.display.items()));
        let damaged: Vec<Rect> = match old {
            None => vec![painted.display.bounds],
            Some(_) if redrawn.is_some() => redrawn.unwrap_or_default().rects().to_vec(),
            Some(old) if children(&old.display).eq(children(&painted.display)) => {
                vec![old.own, painted.own]
            }
            Some(old) => vec![old.display.bounds, painted.display.bounds],
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

fn absolute(rect: Rect, state: PainterState) -> Rect {
    rect.intersect(state.space_clip).translate(state.origin)
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

fn redrawn_damage(old: &[DisplayItem], new: &[DisplayItem]) -> Option<Region> {
    if old.len() != new.len() {
        return None;
    }
    let mut region = Region::NOTHING;
    for pair in old.iter().zip(new) {
        match pair {
            (old, new) if old == new => {}
            (DisplayItem::Main(old), DisplayItem::Main(new))
            | (DisplayItem::Top(old), DisplayItem::Top(new)) => {
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

fn children(display: &Display) -> impl Iterator<Item = (NodeId, Entry)> + '_ {
    display.children().map(|(child, entry, _)| (child, entry))
}

pub(crate) fn paint(doc: &Document, painter: &Painter, rects: &Rects, id: NodeId) {
    let rect = rects.placed(&id).expect("node was not placed").rect;
    let ctx = painter.ctx();
    let own = painter.entered(id, rect);
    let entry = painter.entry(rect);
    let placement = Placement {
        size: rect.size(),
        given: painter.state(),
        own: own.state(),
    };
    let visit = doc.paint_cache.borrow_mut().visit(id, placement, entry);
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
            let local = Rect::from_min_size(Pos2::ZERO, rect.size());
            doc.arena.get(id).paint(doc, &own, rects, local);
            let reads = ctx.swap_space_read(outer);
            let recorded = Recorded {
                reads,
                ..ctx.exit_paint()
            };
            let bounds = recorded.bounds;
            doc.paint_cache
                .borrow_mut()
                .store(id, placement, entry, recorded);
            bounds
        }
    };
    ctx.paint_child(id, entry, entry.place(bounds));
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
