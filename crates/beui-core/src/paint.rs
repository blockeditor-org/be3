use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use crate::damage::Region;
use crate::display::{Display, Item as DisplayItem, Part};
use crate::geometry::{Pos2, Rect, Vec2};
use crate::painter::{Entry, Painter, PainterState, Shape, placed_shape};

use crate::document::Document;
use crate::node::{ANCESTOR_LIMIT, Arena, NodeId, NodeMap, Rects};

#[derive(Clone, PartialEq)]
pub enum Item {
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

pub struct Recorded {
    pub items: Vec<Item>,
    pub own: Rect,
    pub bounds: Rect,
    pub deadline: Option<Instant>,
    pub reads: bool,
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

#[derive(Clone, PartialEq, Debug)]
pub struct Move {
    pub node: NodeId,
    pub moving: Vec<NodeId>,
    pub viewport: Rect,
    pub by: Vec2,
    pub damaged: Region,
    pub absorbed: Vec<Rect>,
}

#[derive(Default)]
pub struct PaintCache {
    moves: Vec<Move>,
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

    pub fn absolute_bounds(&self, id: NodeId) -> Rect {
        self.entries.get(&id).map_or(Rect::NOTHING, |painted| {
            absolute(painted.display.bounds, painted.placement.own)
        })
    }

    pub fn forget(&mut self, id: NodeId) {
        self.entries.remove(&id);
        self.dirt.remove(&id);
        self.deadlines.remove(&id);
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn mark(&mut self, ids: &[NodeId], arena: &Arena) {
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

    pub fn take_due(&mut self, now: Instant) -> Vec<NodeId> {
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

    pub fn next_deadline(&self) -> Option<Instant> {
        self.deadlines.values().min().copied()
    }

    pub fn settle_roots(&mut self, roots: Vec<NodeId>) {
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

    pub fn take_damage(&mut self) -> Region {
        std::mem::take(&mut self.damage)
    }

    pub fn take_moves(&mut self) -> Vec<Move> {
        std::mem::take(&mut self.moves)
    }

    pub fn take_recorded(&mut self) -> bool {
        std::mem::take(&mut self.recorded)
    }

    pub fn rooted(&self) -> Vec<(NodeId, Rc<Display>, Entry)> {
        self.roots
            .iter()
            .filter_map(|(root, _)| {
                let painted = self.entries.get(root)?;
                Some((*root, Rc::clone(&painted.display), painted.entry))
            })
            .collect()
    }

    pub fn painting(&self) -> Vec<(Rc<Display>, Entry)> {
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
        let entered: Vec<NodeId> = painted
            .display
            .children()
            .map(|(child, ..)| child)
            .collect();
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
            Some(old) => match scrolled(&old, &painted).or_else(|| reflowed(&old, &painted)) {
                Some(Moving {
                    by,
                    viewport,
                    damaged,
                    moving,
                    absorbed,
                }) => {
                    let mut region = Region::NOTHING;
                    for rect in damaged {
                        region.add(absolute(rect, state));
                    }
                    self.moves.push(Move {
                        node: id,
                        moving,
                        viewport: absolute(viewport, state),
                        by,
                        damaged: region,
                        absorbed: absorbed
                            .into_iter()
                            .map(|rect| absolute(rect, state))
                            .collect(),
                    });
                    Vec::new()
                }
                None => vec![old.display.bounds, painted.display.bounds],
            },
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

struct Moving {
    by: Vec2,
    viewport: Rect,
    damaged: Vec<Rect>,
    moving: Vec<NodeId>,
    absorbed: Vec<Rect>,
}

fn reflowed(old: &Painted, new: &Painted) -> Option<Moving> {
    if new.placement.own.space.is_none()
        || !old.placement.own.sees(new.placement.own)
        || !old.placement.own.settles(new.placement.own)
    {
        return None;
    }
    let (old, new) = (&old.display, &new.display);
    if old.shapes != new.shapes {
        return None;
    }
    let before: Vec<_> = old.children().collect();
    let after: Vec<_> = new.children().collect();
    let clip = after.first().or(before.first())?.1.clip;
    if before
        .iter()
        .chain(after.iter())
        .any(|(_, entry, _)| entry.clip != clip || entry.shift.is_some())
    {
        return None;
    }
    let kept: Vec<NodeId> = after
        .iter()
        .filter(|(id, ..)| before.iter().any(|(held, ..)| held == id))
        .map(|(id, ..)| *id)
        .collect();
    let held: Vec<NodeId> = before
        .iter()
        .filter(|(id, ..)| kept.contains(id))
        .map(|(id, ..)| *id)
        .collect();
    if kept != held {
        return None;
    }
    let previous = |id: NodeId| {
        before
            .iter()
            .find(|(held, ..)| *held == id)
            .map(|(_, entry, display)| (*entry, Rc::clone(display)))
    };
    let mut by = None;
    let mut moving = Vec::new();
    for (id, entry, _) in &after {
        let Some((was, _)) = previous(*id) else {
            continue;
        };
        let delta = entry.translation - was.translation;
        if delta == Vec2::ZERO {
            if !moving.is_empty() {
                return None;
            }
            continue;
        }
        if by.is_some_and(|by| by != delta) {
            return None;
        }
        by = Some(delta);
        moving.push(*id);
    }
    let by = by?;
    if by.x != 0.0 && by.y != 0.0 {
        return None;
    }
    let mut changed = Rect::NOTHING;
    for (id, entry, child) in &after {
        if !kept.contains(id) || moving.contains(id) {
            changed = changed.union(entry.place(child.bounds));
        }
    }
    for (id, entry, child) in &before {
        if !kept.contains(id) || moving.contains(id) {
            changed = changed.union(entry.place(child.bounds));
        }
    }
    let viewport = changed.intersect(clip);
    if !viewport.is_positive() {
        return None;
    }
    let within = |rect: Rect| rect.intersect(viewport);
    let mut damaged = uncovered(viewport, viewport.translate(by));
    let mut absorbed = Vec::new();
    for (id, entry, child) in &after {
        let placed = entry.place(child.bounds);
        match previous(*id) {
            None => {
                damaged.push(within(placed));
                absorbed.push(placed);
                moving.push(*id);
            }
            Some((_, held)) if !Rc::ptr_eq(&held, child) => {
                damaged.push(within(placed));
                damaged.push(within(placed.translate(-by)));
            }
            Some(_) if !moving.contains(id) => damaged.push(within(placed)),
            Some(_) => {}
        }
    }
    for (id, entry, child) in &before {
        if !kept.contains(id) {
            absorbed.push(entry.place(child.bounds));
        }
    }
    Some(Moving {
        by,
        viewport,
        damaged,
        moving,
        absorbed,
    })
}

fn scrolled(old: &Painted, new: &Painted) -> Option<Moving> {
    if !old.placement.own.sees(new.placement.own) || !old.placement.own.settles(new.placement.own) {
        return None;
    }
    let (old, new) = (&old.display, &new.display);
    if unshifted(old) != unshifted(new) {
        return None;
    }
    let content = |display: &Display| {
        display
            .children()
            .filter_map(|(id, entry, child)| Some((id, entry, entry.shift?, Rc::clone(child))))
            .collect::<Vec<_>>()
    };
    let (before, after) = (content(old), content(new));
    let (first, last) = (before.first()?, after.first()?);
    let (viewport, shift) = (first.1.clip, last.2);
    let by = shift - first.2;
    if by == Vec2::ZERO
        || before
            .iter()
            .any(|(_, entry, held, _)| entry.clip != viewport || *held != first.2)
        || after
            .iter()
            .any(|(_, entry, held, _)| entry.clip != viewport || *held != shift)
    {
        return None;
    }
    let kept: Vec<NodeId> = after
        .iter()
        .filter(|(id, ..)| before.iter().any(|(held, ..)| held == id))
        .map(|(id, ..)| *id)
        .collect();
    let held: Vec<NodeId> = before
        .iter()
        .filter(|(id, ..)| kept.contains(id))
        .map(|(id, ..)| *id)
        .collect();
    if kept.is_empty() || kept != held {
        return None;
    }
    for (id, entry, ..) in &after {
        if let Some((_, previous, ..)) = before.iter().find(|(held, ..)| held == id)
            && entry.translation - previous.translation != by
        {
            return None;
        }
    }
    let within = |rect: Rect| rect.intersect(viewport);
    let mut damaged = uncovered(viewport, viewport.translate(by));
    for (id, entry, _, child) in &after {
        if !kept.contains(id) {
            damaged.push(within(entry.place(child.bounds)));
        }
    }
    for (id, entry, _, child) in &before {
        if !kept.contains(id) {
            damaged.push(within(entry.place(child.bounds).translate(by)));
        }
    }
    for rect in fixed(new) {
        damaged.push(within(rect));
        damaged.push(within(rect.translate(by)));
    }
    Some(Moving {
        by,
        viewport,
        damaged,
        moving: after.iter().map(|(id, ..)| *id).collect(),
        absorbed: Vec::new(),
    })
}

fn unshifted(display: &Display) -> Vec<DisplayItem<'_>> {
    display
        .items()
        .into_iter()
        .filter(|item| !matches!(item, DisplayItem::Child(_, Entry { shift: Some(_), .. })))
        .collect()
}

fn fixed(display: &Display) -> Vec<Rect> {
    display
        .parts
        .iter()
        .flat_map(|part| match part {
            Part::Shapes { start, end, .. } => display.shapes[*start as usize..*end as usize]
                .iter()
                .map(crate::damage::bounds)
                .collect::<Vec<_>>(),
            Part::Child(_, entry, child) if entry.shift.is_none() => {
                vec![entry.place(child.bounds)]
            }
            Part::Child(..) => Vec::new(),
        })
        .collect()
}

pub fn uncovered(rect: Rect, cover: Rect) -> Vec<Rect> {
    let cover = rect.intersect(cover);
    if !cover.is_positive() {
        return vec![rect];
    }
    let rows = [
        (rect.top(), cover.top()),
        (cover.top(), cover.bottom()),
        (cover.bottom(), rect.bottom()),
    ];
    let mut left = Vec::new();
    for (index, (top, bottom)) in rows.into_iter().enumerate() {
        if top >= bottom {
            continue;
        }
        let spans: &[(f32, f32)] = match index {
            1 => &[(rect.left(), cover.left()), (cover.right(), rect.right())],
            _ => &[(rect.left(), rect.right())],
        };
        for (from, to) in spans {
            if from < to {
                left.push(Rect::from_min_max(
                    Pos2::new(*from, top),
                    Pos2::new(*to, bottom),
                ));
            }
        }
    }
    left
}

pub fn fixed_damage(
    painting: &[(NodeId, Rc<Display>, Entry)],
    scroll: NodeId,
    moving: &[NodeId],
    viewport: Rect,
    by: Vec2,
) -> (Region, Rect) {
    let mut fixed = Fixed {
        scroll,
        moving,
        viewport,
        by,
        damage: Region::NOTHING,
        inner: viewport,
    };
    for (root, display, entry) in painting {
        for top in [false, true] {
            fixed.within(display, *entry, *root == scroll, top);
        }
    }
    (fixed.damage, fixed.inner)
}

struct Fixed<'a> {
    scroll: NodeId,
    moving: &'a [NodeId],
    viewport: Rect,
    by: Vec2,
    damage: Region,
    inner: Rect,
}

impl Fixed<'_> {
    fn within(&mut self, display: &Display, at: Entry, scrolls: bool, top: bool) {
        if !at.place(display.bounds).intersects(self.viewport) {
            return;
        }
        for part in display.parts.iter() {
            match part {
                Part::Shapes {
                    top: on_top,
                    start,
                    end,
                } => {
                    if *on_top != top {
                        continue;
                    }
                    for shape in &display.shapes[*start as usize..*end as usize] {
                        self.shape(&placed_shape(shape, at.translation, at.clip));
                    }
                }
                Part::Child(id, _, _) if scrolls && self.moving.contains(id) => {}
                Part::Child(id, entry, child) => {
                    self.within(child, at.compose(*entry), *id == self.scroll, top);
                }
            }
        }
    }

    fn shape(&mut self, shape: &Shape) {
        let bounds = crate::damage::bounds(shape);
        if !bounds.intersects(self.viewport) {
            return;
        }
        if covers(shape, self.viewport) {
            if matches!(shape, Shape::Rect { color, .. } if color.alpha() == u8::MAX) {
                self.damage = Region::NOTHING;
            }
            return;
        }
        if let Some(interior) = rim(shape)
            && bounds.contains_rect(self.viewport)
        {
            self.inner = self.inner.intersect(interior);
            return;
        }
        for rect in painted(shape, bounds) {
            self.damage.add(rect.intersect(self.viewport));
            self.damage
                .add(rect.translate(self.by).intersect(self.viewport));
        }
    }
}

fn rim(shape: &Shape) -> Option<Rect> {
    match shape {
        Shape::Rect {
            rect,
            corner_radius,
            stroke_width,
            rotation,
            ..
        } if *stroke_width > 0.0 && !rotation.turns() => {
            Some(rect.shrink(stroke_width + corner_radius + 1.0))
        }
        _ => None,
    }
}

pub fn painted(shape: &Shape, bounds: Rect) -> Vec<Rect> {
    match rim(shape) {
        Some(interior) => uncovered(bounds, interior),
        None => vec![bounds],
    }
}

pub fn covers(shape: &Shape, viewport: Rect) -> bool {
    let Shape::Rect {
        rect,
        corner_radius,
        stroke_width,
        rotation,
        clip,
        ..
    } = shape
    else {
        return false;
    };
    *stroke_width == 0.0
        && !rotation.turns()
        && clip.contains_rect(viewport)
        && rect.shrink(*corner_radius).contains_rect(viewport)
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

pub fn redrawn_in_place(old: &Shape, new: &Shape) -> Option<Region> {
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

pub fn paint(doc: &Document, painter: &Painter, rects: &Rects, id: NodeId) {
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

pub fn same_shape(left: &Shape, right: &Shape) -> bool {
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
