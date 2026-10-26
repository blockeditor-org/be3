use std::collections::HashMap;
use std::rc::Rc;

use crate::context::Moved;
use crate::damage::{Region, bounds};
use crate::display::{Display, Group, Part};
use crate::fade::Fade;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::node::NodeId;
use crate::painter::{Shape, placed_shape};
use crate::window::WindowId;

type Key = (NodeId, u32);

pub struct Check {
    pub window: WindowId,
    pub viewport: Rect,
    pub previous: Vec<(Rc<Display>, Vec2)>,
    pub region: Region,
    pub moved: Option<Moved>,
}

pub struct Seen {
    nodes: HashMap<Key, Node>,
    roots: Vec<Key>,
    cover: Option<usize>,
}

struct Node {
    shapes: Vec<(usize, Shape)>,
    fades: Vec<Fade>,
    children: Vec<Key>,
    subtree: (usize, Rect),
}

impl Seen {
    pub fn of(painting: &[(Rc<Display>, Vec2)], area: Option<Rect>) -> Self {
        let mut walk = Walk {
            seen: Self {
                nodes: HashMap::new(),
                roots: Vec::new(),
                cover: None,
            },
            area,
            occurrences: HashMap::new(),
            count: 0,
        };
        for (display, at) in painting {
            let key = walk.node(display, Place::at(*at), Rect::EVERYTHING, &[]);
            walk.seen.roots.push(key);
        }
        walk.seen
    }

    pub fn shapes(&self) -> impl Iterator<Item = &Shape> {
        self.nodes
            .values()
            .flat_map(|node| node.shapes.iter().map(|(_, shape)| shape))
    }
}

struct Walk {
    seen: Seen,
    area: Option<Rect>,
    occurrences: HashMap<NodeId, u32>,
    count: usize,
}

#[derive(Clone, Copy)]
struct Place {
    at: Vec2,
    scale: f32,
}

impl Place {
    fn at(at: Vec2) -> Self {
        Self { at, scale: 1.0 }
    }

    fn rect(self, rect: Rect) -> Rect {
        rect.scaled(self.scale).translate(self.at)
    }

    fn within(self, offset: Vec2, scale: f32) -> Self {
        Self {
            at: self.at + offset * self.scale,
            scale: self.scale * scale,
        }
    }

    fn shape(self, shape: &Shape, clip: Rect) -> Shape {
        match self.scale == 1.0 {
            true => placed_shape(shape, self.at, clip),
            false => {
                let mut scaled = shape.clone();
                crate::context::scale_shape(&mut scaled, self.scale);
                placed_shape(&scaled, self.at, clip)
            }
        }
    }
}

impl Walk {
    fn node(&mut self, display: &Display, at: Place, clip: Rect, fades: &[Fade]) -> Key {
        let occurrence = self.occurrences.entry(display.node).or_insert(0);
        let key = (display.node, *occurrence);
        *occurrence += 1;
        let mut node = Node {
            shapes: Vec::new(),
            fades: fades.to_vec(),
            children: Vec::new(),
            subtree: (self.count, at.rect(display.bounds).intersect(clip)),
        };
        self.parts(display, &display.parts, at, clip, fades, &mut node);
        self.seen.nodes.insert(key, node);
        key
    }

    fn parts(
        &mut self,
        display: &Display,
        parts: &[Part],
        at: Place,
        clip: Rect,
        fades: &[Fade],
        node: &mut Node,
    ) {
        for part in parts {
            match part {
                Part::Shapes { start, end, .. } => {
                    for shape in &display.shapes[*start as usize..*end as usize] {
                        let placed = at.shape(shape, clip);
                        let index = self.count;
                        self.count += 1;
                        if let Some(area) = self.area
                            && fades.iter().all(|fade| fade.is_none())
                            && opaque_cover(&placed, area)
                        {
                            self.seen.cover = Some(index);
                        }
                        node.shapes.push((index, placed));
                    }
                }
                Part::Child(_, translation, child) => {
                    let key = self.node(child, at.within(*translation, 1.0), clip, fades);
                    node.children.push(key);
                }
                Part::Group(group, inner) => match *group {
                    Group::Clip(rect) => {
                        let clip = clip.intersect(at.rect(rect));
                        self.parts(display, inner, at, clip, fades, node);
                    }
                    Group::Scroll {
                        viewport,
                        offset,
                        scale,
                        ..
                    } => {
                        let clip = clip.intersect(at.rect(viewport));
                        self.parts(display, inner, at.within(offset, scale), clip, fades, node);
                    }
                    Group::Transform { offset, scale } => {
                        self.parts(display, inner, at.within(offset, scale), clip, fades, node);
                    }
                    Group::Mask { fade, .. } => {
                        let mut within = fades.to_vec();
                        within.push(fade.scaled(at.scale, at.at));
                        self.parts(display, inner, at, clip, &within, node);
                    }
                    Group::Filter(_) => self.parts(display, inner, at, clip, fades, node),
                },
            }
        }
    }
}

fn opaque_cover(shape: &Shape, area: Rect) -> bool {
    matches!(shape, Shape::Rect { color, .. } if color.alpha() == u8::MAX)
        && super::covers(shape, area)
}

pub struct Changes {
    pub needed: Vec<(Rect, Why)>,
}

#[derive(Clone, Copy, Debug)]
pub enum Why {
    Added(NodeId),
    Removed(NodeId),
    Faded(NodeId),
    Changed(NodeId),
    Restacked(NodeId),
}

struct Compare<'a> {
    old: &'a Seen,
    new: &'a Seen,
    moved: Option<Moved>,
    needed: Vec<(Rect, Why)>,
}

pub fn changes(old: &Seen, new: &Seen, moved: Option<Moved>) -> Changes {
    let mut compare = Compare {
        old,
        new,
        moved,
        needed: Vec::new(),
    };
    for (key, now) in &new.nodes {
        match old.nodes.get(key) {
            Some(was) => compare.node(key.0, was, now),
            None => {
                for (index, shape) in &now.shapes {
                    for rect in footprint(shape) {
                        compare.need_new(rect, *index, Why::Added(key.0));
                    }
                }
            }
        }
    }
    for (key, was) in &old.nodes {
        if !new.nodes.contains_key(key) {
            for (index, shape) in &was.shapes {
                for rect in footprint(shape) {
                    compare.need_old(rect, *index, Why::Removed(key.0));
                }
            }
        }
    }
    compare.restacked(&old.roots, &new.roots);
    Changes {
        needed: compare.needed,
    }
}

impl Compare<'_> {
    fn landed(&self) -> Option<(Rect, Vec2)> {
        self.moved.map(|moved| (moved.to(), moved.by))
    }

    fn visible_old(&self, index: usize) -> bool {
        self.old.cover.is_none_or(|cover| index >= cover)
    }

    fn visible_new(&self, index: usize) -> bool {
        self.new.cover.is_none_or(|cover| index >= cover)
    }

    fn need(&mut self, rects: impl IntoIterator<Item = Rect>, why: Why) {
        self.needed
            .extend(rects.into_iter().map(|rect| (rect, why)));
    }

    fn outside(&self, rect: Rect) -> Vec<Rect> {
        match self.landed() {
            Some((landed, _)) => super::uncovered(rect, landed),
            None => vec![rect],
        }
    }

    fn need_old(&mut self, rect: Rect, index: usize, why: Why) {
        self.need(self.outside(rect), why);
        if let Some((landed, by)) = self.landed()
            && self.visible_old(index)
        {
            self.need([rect.translate(by).intersect(landed)], why);
        }
    }

    fn need_new(&mut self, rect: Rect, index: usize, why: Why) {
        self.need(self.outside(rect), why);
        if let Some((landed, _)) = self.landed()
            && self.visible_new(index)
        {
            self.need([rect.intersect(landed)], why);
        }
    }

    fn node(&mut self, id: NodeId, was: &Node, now: &Node) {
        let reach = |shapes: &[&[(usize, Shape)]]| {
            shapes
                .iter()
                .flat_map(|shapes| shapes.iter())
                .fold(Rect::NOTHING, |reach, (_, shape)| {
                    reach.union(bounds(shape))
                })
        };
        if was.fades != now.fades {
            let reach = reach(&[&was.shapes, &now.shapes]);
            for fade in fade_changes(&was.fades, &now.fades) {
                let rects = self.outside(fade.intersect(reach));
                self.need(rects, Why::Faded(id));
            }
        }
        for rect in shape_changes(&was.shapes, &now.shapes, |old, new| old.same_look(new)) {
            let rects = self.outside(rect);
            self.need(rects, Why::Changed(id));
        }
        if let Some((landed, by)) = self.landed() {
            let shifted: Vec<(usize, Shape)> = was
                .shapes
                .iter()
                .filter(|(index, _)| self.visible_old(*index))
                .map(|(index, shape)| (*index, placed_shape(shape, by, Rect::EVERYTHING)))
                .collect();
            let shown: Vec<(usize, Shape)> = now
                .shapes
                .iter()
                .filter(|(index, _)| self.visible_new(*index))
                .cloned()
                .collect();
            let shifted_fades: Vec<Fade> =
                was.fades.iter().map(|fade| fade.translate(by)).collect();
            if shifted_fades != now.fades {
                let reach = reach(&[&shifted, &shown]);
                for fade in fade_changes(&shifted_fades, &now.fades) {
                    self.need([fade.intersect(reach).intersect(landed)], Why::Faded(id));
                }
            }
            for rect in shape_changes(&shifted, &shown, |old, new| moved_same(old, new, landed)) {
                self.need([rect.intersect(landed)], Why::Changed(id));
            }
        }
        self.restacked(&was.children, &now.children);
    }

    fn restacked(&mut self, was: &[Key], now: &[Key]) {
        let positions: HashMap<Key, usize> = was
            .iter()
            .enumerate()
            .map(|(position, key)| (*key, position))
            .collect();
        let kept: Vec<(Key, usize)> = now
            .iter()
            .filter_map(|key| positions.get(key).map(|position| (*key, *position)))
            .collect();
        let order: Vec<usize> = kept.iter().map(|(_, position)| *position).collect();
        for ((key, _), stays) in kept.iter().zip(increasing(&order)) {
            if stays {
                continue;
            }
            let (index, rect) = self.old.nodes[key].subtree;
            self.need_old(rect, index, Why::Restacked(key.0));
            let (index, rect) = self.new.nodes[key].subtree;
            self.need_new(rect, index, Why::Restacked(key.0));
        }
    }
}

fn fade_changes(was: &[Fade], now: &[Fade]) -> Vec<Rect> {
    let mut changed = Vec::new();
    for index in 0..was.len().max(now.len()) {
        let (old, new) = (was.get(index), now.get(index));
        if old == new {
            continue;
        }
        match (old, new) {
            (Some(old), Some(new)) => {
                changed.extend(old.reach(*new));
                changed.extend(new.reach(*old));
            }
            (Some(fade), None) | (None, Some(fade)) => changed.extend(fade.reach(Fade::NONE)),
            (None, None) => {}
        }
    }
    changed
}

fn shape_changes(
    was: &[(usize, Shape)],
    now: &[(usize, Shape)],
    same: impl Fn(&Shape, &Shape) -> bool,
) -> Vec<Rect> {
    let mut changed = Vec::new();
    for index in 0..was.len().max(now.len()) {
        match (was.get(index), now.get(index)) {
            (Some((_, old)), Some((_, new))) if same(old, new) => {}
            (Some((_, old)), Some((_, new))) => match super::redrawn_in_place(old, new) {
                Some(region) => changed.extend(region.rects().iter().copied()),
                None => changed.extend(footprint(old).into_iter().chain(footprint(new))),
            },
            (Some((_, shape)), None) | (None, Some((_, shape))) => changed.extend(footprint(shape)),
            (None, None) => {}
        }
    }
    changed
}

fn footprint(shape: &Shape) -> Vec<Rect> {
    super::painted(shape, bounds(shape))
}

fn moved_same(old: &Shape, new: &Shape, landed: Rect) -> bool {
    let uniform = |shape: &Shape| super::covers(shape, landed);
    let fill = |shape: &Shape| match shape {
        Shape::Rect { color, .. } => Some(*color),
        _ => None,
    };
    (uniform(old) && uniform(new) && fill(old) == fill(new))
        || (unclipped(old).same_look(&unclipped(new))
            && clip_of(old).intersect(landed) == clip_of(new).intersect(landed))
}

fn unclipped(shape: &Shape) -> Shape {
    let mut shape = shape.clone();
    *crate::context::shape_clip(&mut shape) = Rect::EVERYTHING;
    shape
}

fn clip_of(shape: &Shape) -> Rect {
    *crate::context::shape_clip(&mut shape.clone())
}

fn increasing(order: &[usize]) -> Vec<bool> {
    let mut tails: Vec<usize> = Vec::new();
    let mut ends: Vec<usize> = Vec::new();
    let mut previous = vec![None; order.len()];
    for (index, value) in order.iter().enumerate() {
        let at = tails.partition_point(|tail| tail < value);
        if at > 0 {
            previous[index] = Some(ends[at - 1]);
        }
        match at == tails.len() {
            true => {
                tails.push(*value);
                ends.push(index);
            }
            false => {
                tails[at] = *value;
                ends[at] = index;
            }
        }
    }
    let mut kept = vec![false; order.len()];
    let mut current = ends.last().copied();
    while let Some(index) = current {
        kept[index] = true;
        current = previous[index];
    }
    kept
}

pub fn covered(rect: Rect, region: &Region, pixels_per_point: f32) -> bool {
    let Some(rect) = pixels(rect, pixels_per_point) else {
        return true;
    };
    let mut left = vec![rect];
    for damaged in region.rects() {
        let Some(damaged) = pixels(*damaged, pixels_per_point) else {
            continue;
        };
        left = left
            .into_iter()
            .flat_map(|rect| super::uncovered(rect, damaged))
            .collect();
    }
    left.iter().all(|rect| !rect.is_positive())
}

const ROUNDING: f32 = 1.0e-3;

fn pixels(rect: Rect, pixels_per_point: f32) -> Option<Rect> {
    if !rect.is_positive() {
        return None;
    }
    let pixels = Rect::from_min_max(
        Pos2::new(
            (rect.min.x * pixels_per_point + ROUNDING).floor(),
            (rect.min.y * pixels_per_point + ROUNDING).floor(),
        ),
        Pos2::new(
            (rect.max.x * pixels_per_point - ROUNDING).ceil(),
            (rect.max.y * pixels_per_point - ROUNDING).ceil(),
        ),
    );
    pixels.is_positive().then_some(pixels)
}

pub fn changed_area(old: &Seen, new: &Seen) -> Region {
    let key = |shape: &Shape| {
        let rect = bounds(shape);
        (
            std::mem::discriminant(shape),
            [rect.min.x, rect.min.y, rect.max.x, rect.max.y].map(f32::to_bits),
        )
    };
    let mut held: HashMap<_, Vec<&Shape>> = HashMap::new();
    for shape in old.shapes() {
        held.entry(key(shape)).or_default().push(shape);
    }
    let mut changed = Region::NOTHING;
    for shape in new.shapes() {
        let bucket = held.entry(key(shape)).or_default();
        match bucket.iter().position(|old| old.same_look(shape)) {
            Some(at) => {
                bucket.swap_remove(at);
            }
            None => changed.add(bounds(shape)),
        }
    }
    for shape in held.into_values().flatten() {
        changed.add(bounds(shape));
    }
    changed
}
