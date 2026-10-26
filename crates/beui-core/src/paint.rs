use foldhash::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Instant;

use crate::damage::{Region, WindowRegions};
use crate::display::{Display, Group, Part};
use crate::document::Document;
use crate::fade::Fade;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::node::{ANCESTOR_LIMIT, Arena, NodeId, NodeMap, Rects, SpaceId};
use crate::painter::{Painter, PainterState, Scope, Shape, placed_shape};
use crate::window::WindowId;

pub mod verify;

pub struct Item {
    pub scope: Option<Rc<Scope>>,
    pub recording: Recording,
}

pub enum Recording {
    Shape(Shape),
    Child(NodeId, Vec2),
}

#[derive(Clone, Copy)]
struct Placement {
    size: Vec2,
    own: PainterState,
}

pub struct Recorded {
    pub items: Vec<Item>,
    pub deadline: Option<Instant>,
    pub reads: bool,
}

struct Painted {
    placement: Placement,
    at: Vec2,
    display: Rc<Display>,
    reads: bool,
    below: bool,
    given: Option<HashMap<NodeId, PainterState>>,
}

#[derive(Default)]
struct Dirt {
    own: bool,
    below: bool,
    listed: bool,
    dirty: Vec<NodeId>,
}

enum Visit {
    Reuse,
    Descend {
        children: Vec<(NodeId, PainterState)>,
        every: bool,
    },
    Record,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Move {
    pub window: WindowId,
    pub space: SpaceId,
    pub viewport: Rect,
    pub by: Vec2,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Stale {
    pub window: WindowId,
    pub rect: Rect,
    pub scroll: Option<SpaceId>,
}

#[derive(Default)]
pub struct PaintCache {
    moves: Vec<Move>,
    entries: NodeMap<Painted>,
    dirt: NodeMap<Dirt>,
    deadlines: HashMap<NodeId, Instant>,
    roots: Vec<(NodeId, Rect)>,
    places: HashMap<NodeId, (Vec2, Rect)>,
    scrolls: HashMap<SpaceId, Option<SpaceId>>,
    root_windows: HashMap<NodeId, WindowId>,
    window: WindowId,
    damage: WindowRegions,
    stale: Vec<Stale>,
    recorded: bool,
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
        for id in ids {
            self.mark_one(*id, arena);
        }
    }

    fn mark_one(&mut self, id: NodeId, arena: &Arena) {
        if !arena.current(id) {
            return;
        }
        self.dirt.get_or_default(id).own = true;
        let mut child = id;
        for _ in 0..ANCESTOR_LIMIT {
            let Some(node) = arena.parent(child) else {
                return;
            };
            let listed = &mut self.dirt.get_or_default(child).listed;
            if *listed {
                return;
            }
            *listed = true;
            let dirt = self.dirt.get_or_default(node);
            dirt.below = true;
            dirt.dirty.push(child);
            child = node;
        }
    }

    fn take_dirt(&mut self, id: NodeId) -> Dirt {
        let dirt = self.dirt.remove(&id).unwrap_or_default();
        for child in &dirt.dirty {
            if let Some(held) = self.dirt.get_mut(child) {
                held.listed = false;
            }
        }
        dirt
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

    pub fn settle_roots(&mut self, windows: &[(WindowId, NodeId)]) {
        let window_of = |id: NodeId| {
            windows
                .iter()
                .find(|(_, root)| *root == id)
                .map_or(WindowId::MAIN, |(window, _)| *window)
        };
        let roots: Vec<(NodeId, Rect)> = windows
            .iter()
            .map(|(_, id)| (*id, self.absolute_bounds(*id)))
            .collect();
        let places: HashMap<NodeId, (Vec2, Rect)> = roots
            .iter()
            .filter_map(|(id, _)| {
                let own = self.entries.get(id)?.placement.own;
                Some((*id, (own.origin, own.space_clip)))
            })
            .collect();
        for (id, bounds) in &roots {
            if let Some((_, held)) = self.roots.iter().find(|(root, _)| root == id)
                && held != bounds
            {
                if self.places.get(id) != places.get(id) {
                    self.stale.push(Stale {
                        window: window_of(*id),
                        rect: *held,
                        scroll: None,
                    });
                    self.damage.add(window_of(*id), *bounds);
                }
                self.recorded = true;
            }
        }
        self.places = places;
        for (id, bounds) in &roots {
            let Some(was) = self.root_windows.get(id).copied() else {
                continue;
            };
            let window = window_of(*id);
            if was == window {
                continue;
            }
            if let Some((_, held)) = self.roots.iter().find(|(root, _)| root == id) {
                self.stale.push(Stale {
                    window: was,
                    rect: *held,
                    scroll: None,
                });
            }
            self.damage.add(window, *bounds);
            self.recorded = true;
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
            for (id, bounds) in added.chain(&after[reordered..]) {
                self.damage.add(window_of(*id), *bounds);
            }
            for (id, bounds) in removed.chain(&before[reordered..]) {
                self.stale.push(Stale {
                    window: self.root_windows.get(id).copied().unwrap_or(WindowId::MAIN),
                    rect: *bounds,
                    scroll: None,
                });
            }
            self.recorded = true;
        }
        self.roots = roots;
        self.root_windows = windows.iter().map(|(window, id)| (*id, *window)).collect();
    }

    pub fn take_damage(&mut self) -> WindowRegions {
        std::mem::take(&mut self.damage)
    }

    pub fn paint_into(&mut self, window: WindowId) {
        self.window = window;
    }

    pub fn take_stale(&mut self) -> Vec<Stale> {
        std::mem::take(&mut self.stale)
    }

    pub fn take_moves(&mut self) -> Vec<Move> {
        std::mem::take(&mut self.moves)
    }

    pub fn take_recorded(&mut self) -> bool {
        std::mem::take(&mut self.recorded)
    }

    pub fn scrolls_within(&self, scroll: Option<SpaceId>, space: SpaceId) -> bool {
        let mut current = scroll;
        for _ in 0..ANCESTOR_LIMIT {
            match current {
                Some(held) if held == space => return true,
                Some(held) => current = self.scrolls.get(&held).copied().flatten(),
                None => return false,
            }
        }
        false
    }

    pub fn painting_of(&self, root: NodeId) -> Option<(Rc<Display>, Vec2)> {
        let painted = self.entries.get(&root)?;
        Some((Rc::clone(&painted.display), painted.at))
    }

    fn visit(&mut self, id: NodeId, placement: Placement, at: Vec2, rects: &Rects) -> Visit {
        let dirt = self.take_dirt(id);
        let Some(painted) = self.entries.get_mut(&id).filter(|painted| {
            !dirt.own
                && painted.placement.size == placement.size
                && painted.placement.own.settles(placement.own)
                && painted.placement.own.raster == placement.own.raster
        }) else {
            return Visit::Record;
        };
        let sees = painted.placement.own.sees(placement.own);
        if painted.reads && !sees {
            return Visit::Record;
        }
        let same = painted.placement.own == placement.own;
        painted.placement = placement;
        painted.at = at;
        if !same {
            painted.given = None;
        }
        if !dirt.below && (sees || !painted.below) {
            return Visit::Reuse;
        }
        if same {
            let given = painted.given.get_or_insert_with(|| {
                let mut children = Vec::new();
                given(&painted.display.parts, placement.own, &mut children);
                children.into_iter().collect()
            });
            let mut children = Vec::with_capacity(dirt.dirty.len());
            for child in &dirt.dirty {
                match given.get(child) {
                    Some(state) => children.push((*child, *state)),
                    None if rects.contains_key(child) => return Visit::Record,
                    None => {}
                }
            }
            return Visit::Descend {
                children,
                every: false,
            };
        }
        let mut children = Vec::new();
        given(&painted.display.parts, placement.own, &mut children);
        if children
            .iter()
            .any(|(child, _)| !self.entries.contains_key(child))
        {
            return Visit::Record;
        }
        Visit::Descend {
            children,
            every: true,
        }
    }

    fn settle_descended(
        &mut self,
        id: NodeId,
        visited: &[(NodeId, Option<Rc<Display>>)],
        every: bool,
    ) {
        let Some(painted) = self.entries.get(&id) else {
            return;
        };
        let held = Rc::clone(&painted.display);
        let changed = visited.iter().any(|(child, before)| {
            let now = self.entries.get(child).map(|painted| &painted.display);
            match (before, now) {
                (Some(before), Some(now)) => !Rc::ptr_eq(before, now),
                (None, None) => false,
                _ => true,
            }
        });
        let rebuilt = match every || changed {
            true => held.with_children(&|child| self.display(child)),
            false => None,
        };
        let below = match every {
            true => self.reads_below(&held),
            false => {
                painted.below
                    || visited.iter().any(|(child, _)| {
                        self.entries
                            .get(child)
                            .is_some_and(|painted| painted.reads || painted.below)
                    })
            }
        };
        if let Some(painted) = self.entries.get_mut(&id) {
            if let Some(rebuilt) = rebuilt {
                painted.display = Rc::new(rebuilt);
                self.recorded = true;
            }
            painted.below = below;
        }
    }

    fn reads_below(&self, display: &Display) -> bool {
        display.children().into_iter().any(|(child, ..)| {
            self.entries
                .get(&child)
                .is_some_and(|painted| painted.reads || painted.below)
        })
    }

    fn note_scrolls(&mut self, parts: &[Part], around: Option<SpaceId>) {
        for part in parts {
            if let Part::Group(group, inner) = part {
                let within = match group {
                    Group::Scroll { space, .. } => {
                        self.scrolls.insert(*space, around);
                        Some(*space)
                    }
                    _ => around,
                };
                self.note_scrolls(inner, within);
            }
        }
    }

    fn store(
        &mut self,
        document: u64,
        id: NodeId,
        placement: Placement,
        at: Vec2,
        recorded: Recorded,
    ) {
        let (shapes, parts) = build(recorded.items, |child| self.display(child));
        let display = Rc::new(Display::new(document, id, shapes, parts));
        self.note_scrolls(&display.parts, placement.own.scroll);
        let below = self.reads_below(&display);
        let old = self.entries.remove(&id);
        match &old {
            None => self
                .damage
                .add(self.window, absolute(display.bounds, placement.own)),
            Some(old) => self.reconcile(old, placement.own, &display),
        }
        match recorded.deadline {
            Some(deadline) => self.deadlines.insert(id, deadline),
            None => self.deadlines.remove(&id),
        };
        self.entries.insert(
            id,
            Painted {
                placement,
                at,
                display,
                reads: recorded.reads,
                below,
                given: None,
            },
        );
        self.recorded = true;
    }

    fn reconcile(&mut self, old: &Painted, now: PainterState, display: &Display) {
        let still = old.placement.own.sees(now);
        let after = Elements::of(display, now.scroll, &[]);
        let before = Elements::of(&old.display, now.scroll, &after.scrolls);
        let window = self.window;
        let stale = |stale: &mut Vec<Stale>, (rect, scroll): (Rect, Option<SpaceId>)| {
            stale.push(Stale {
                window,
                rect: absolute(rect, now),
                scroll,
            });
        };
        let (runs, mut drawn) = before.own();
        for placed in runs {
            stale(&mut self.stale, placed);
        }
        let (runs, drawings) = after.own();
        for (rect, _) in runs {
            self.damage.add(self.window, absolute(rect, now));
        }
        for drawing in drawings {
            let held = drawn
                .iter()
                .position(|held| held.drawing.same_draw(drawing.drawing))
                .map(|index| drawn.swap_remove(index));
            let redrawn = held
                .as_ref()
                .and_then(|held| after.redrawn(&before, held, &drawing));
            match redrawn {
                Some(region) => {
                    for rect in region.rects() {
                        self.damage.add(self.window, absolute(*rect, now));
                    }
                }
                None => {
                    if let Some(held) = &held {
                        stale(&mut self.stale, before.drawn(held));
                    }
                    self.damage
                        .add(self.window, absolute(after.drawn(&drawing).0, now));
                }
            }
        }
        for held in &drawn {
            stale(&mut self.stale, before.drawn(held));
        }
        let held: HashMap<NodeId, usize> = before
            .children
            .iter()
            .enumerate()
            .map(|(index, child)| (child.id, index))
            .collect();
        let mut kept = Vec::new();
        for (index, child) in after.children.iter().enumerate() {
            let previous = held.get(&child.id).map(|index| &before.children[*index]);
            match previous {
                Some(previous)
                    if previous.at == child.at
                        && same_path(&before.paths[previous.path], &after.paths[child.path]) =>
                {
                    kept.push(index);
                }
                previous => {
                    if let Some(previous) = previous {
                        stale(&mut self.stale, (previous.placed, previous.scroll));
                    }
                    self.damage.add(self.window, absolute(child.placed, now));
                }
            }
        }
        let shown: HashSet<NodeId> = after.children.iter().map(|child| child.id).collect();
        for child in &before.children {
            if !shown.contains(&child.id) {
                stale(&mut self.stale, (child.placed, child.scroll));
            }
        }
        let stayed: HashSet<NodeId> = kept.iter().map(|index| after.children[*index].id).collect();
        let order = before
            .children
            .iter()
            .filter(|child| stayed.contains(&child.id));
        if let Some(from) = order
            .zip(&kept)
            .position(|(was, now)| was.id != after.children[*now].id)
        {
            for index in &kept[from..] {
                let child = &after.children[*index];
                let previous = &before.children[held[&child.id]];
                stale(&mut self.stale, (previous.placed, previous.scroll));
                self.damage.add(self.window, absolute(child.placed, now));
            }
        }
        for mask in &after.masks {
            let Some(held) = before.masks.iter().find(|held| held.space == mask.space) else {
                continue;
            };
            if held.fade == mask.fade {
                continue;
            }
            for band in held.fade.reach(mask.fade) {
                stale(
                    &mut self.stale,
                    (before.place(held.path, band), held.scroll),
                );
            }
            for band in mask.fade.reach(held.fade) {
                self.damage
                    .add(self.window, absolute(after.place(mask.path, band), now));
            }
        }
        for scroll in &after.scrolls {
            let Some(previous) = before
                .scrolls
                .iter()
                .find(|previous| previous.space == scroll.space)
            else {
                continue;
            };
            if previous.offset == scroll.offset && previous.scale == scroll.scale {
                continue;
            }
            let viewport = absolute(scroll.viewport, now);
            match still && previous.viewport == scroll.viewport && previous.scale == scroll.scale {
                true => self.moves.push(Move {
                    window: self.window,
                    space: scroll.space,
                    viewport,
                    by: (scroll.offset - previous.offset) * now.scale,
                }),
                false => {
                    stale(&mut self.stale, (previous.viewport, previous.around));
                    self.damage.add(self.window, viewport);
                }
            }
        }
    }
}

fn given(parts: &[Part], state: PainterState, into: &mut Vec<(NodeId, PainterState)>) {
    for part in parts {
        match part {
            Part::Shapes { .. } => {}
            Part::Child(id, ..) => into.push((*id, state)),
            Part::Group(group, inner) => given(inner, state.within(group), into),
        }
    }
}

struct Elements<'a> {
    display: &'a Display,
    around: Option<SpaceId>,
    paths: Vec<Vec<Group>>,
    shapes: Vec<(usize, u32, u32, Rect)>,
    children: Vec<Placed>,
    scrolls: Vec<Scrolled>,
    masks: Vec<Masked>,
}

struct Placed {
    id: NodeId,
    path: usize,
    at: Vec2,
    placed: Rect,
    scroll: Option<SpaceId>,
}

struct Masked {
    space: SpaceId,
    path: usize,
    fade: Fade,
    scroll: Option<SpaceId>,
}

struct Drawn<'a> {
    path: usize,
    rect: Rect,
    clip: Rect,
    drawing: &'a crate::drawing::Drawing,
}

struct Scrolled {
    space: SpaceId,
    viewport: Rect,
    offset: Vec2,
    scale: f32,
    raster: f32,
    around: Option<SpaceId>,
}

impl<'a> Elements<'a> {
    fn of(display: &'a Display, scroll: Option<SpaceId>, settled: &[Scrolled]) -> Self {
        let mut elements = Self {
            display,
            around: scroll,
            paths: vec![Vec::new()],
            shapes: Vec::new(),
            children: Vec::new(),
            scrolls: Vec::new(),
            masks: Vec::new(),
        };
        elements.walk(&display.parts, 0, scroll, settled);
        elements
    }

    fn walk(&mut self, parts: &[Part], path: usize, scroll: Option<SpaceId>, settled: &[Scrolled]) {
        for part in parts {
            match part {
                Part::Shapes { start, end, bounds } => {
                    let placed = self.place(path, *bounds);
                    self.shapes.push((path, *start, *end, placed));
                }
                Part::Child(id, at, child) => {
                    let placed = self.place(path, child.bounds.translate(*at));
                    self.children.push(Placed {
                        id: *id,
                        path,
                        at: *at,
                        placed,
                        scroll,
                    });
                }
                Part::Group(group, inner) => {
                    let settled_group = match *group {
                        Group::Scroll {
                            space, viewport, ..
                        } => match settled.iter().find(|held| held.space == space) {
                            Some(held) => Group::Scroll {
                                space,
                                viewport,
                                offset: held.offset,
                                scale: held.scale,
                                raster: held.raster,
                            },
                            None => *group,
                        },
                        group => group,
                    };
                    let mut nested = self.paths[path].clone();
                    nested.push(settled_group);
                    self.paths.push(nested);
                    let within = self.paths.len() - 1;
                    let inner_scroll = match *group {
                        Group::Mask { space, fade } => {
                            self.masks.push(Masked {
                                space,
                                path,
                                fade,
                                scroll,
                            });
                            scroll
                        }
                        Group::Scroll {
                            space,
                            viewport,
                            offset,
                            scale,
                            raster,
                        } => {
                            if !self.scrolls.iter().any(|held| held.space == space) {
                                let viewport = self.place(path, viewport);
                                self.scrolls.push(Scrolled {
                                    space,
                                    viewport,
                                    offset,
                                    scale,
                                    raster,
                                    around: scroll,
                                });
                            }
                            Some(space)
                        }
                        _ => scroll,
                    };
                    self.walk(inner, within, inner_scroll, settled);
                }
            }
        }
    }

    fn place(&self, path: usize, rect: Rect) -> Rect {
        self.paths[path]
            .iter()
            .rev()
            .fold(rect, |rect, group| group.place(rect))
    }

    fn scroll_of(&self, path: usize) -> Option<SpaceId> {
        self.paths[path]
            .iter()
            .rev()
            .find_map(|group| match group {
                Group::Scroll { space, .. } => Some(*space),
                _ => None,
            })
            .or(self.around)
    }

    fn own(&self) -> (Vec<(Rect, Option<SpaceId>)>, Vec<Drawn<'a>>) {
        let mut runs = Vec::new();
        let mut drawn = Vec::new();
        for (path, start, end, placed) in &self.shapes {
            let shapes = &self.display.shapes[*start as usize..*end as usize];
            if !shapes
                .iter()
                .any(|shape| matches!(shape, Shape::Drawing { .. }))
            {
                runs.push((*placed, self.scroll_of(*path)));
                continue;
            }
            let mut bounds = Rect::NOTHING;
            for shape in shapes {
                match shape {
                    Shape::Drawing {
                        rect,
                        drawing,
                        clip,
                    } => drawn.push(Drawn {
                        path: *path,
                        rect: *rect,
                        clip: *clip,
                        drawing,
                    }),
                    shape => bounds = bounds.union(crate::damage::bounds(shape)),
                }
            }
            runs.push((self.place(*path, bounds), self.scroll_of(*path)));
        }
        (runs, drawn)
    }

    fn drawn(&self, drawn: &Drawn) -> (Rect, Option<SpaceId>) {
        (
            self.place(drawn.path, drawn.rect.intersect(drawn.clip)),
            self.scroll_of(drawn.path),
        )
    }

    fn redrawn(&self, before: &Self, held: &Drawn, drawn: &Drawn) -> Option<Region> {
        if held.rect != drawn.rect
            || held.clip != drawn.clip
            || !same_path(&before.paths[held.path], &self.paths[drawn.path])
        {
            return None;
        }
        let damage = drawn.drawing.damage_since(held.drawing)?;
        let visible = drawn.rect.intersect(drawn.clip);
        let mut region = Region::NOTHING;
        for damaged in damage.rects() {
            let rect = damaged
                .translate(drawn.rect.min.to_vec2())
                .intersect(visible);
            region.add(self.place(drawn.path, rect));
        }
        Some(region)
    }
}

fn same_path(left: &[Group], right: &[Group]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.same_place(right))
}

fn build(
    items: Vec<Item>,
    child: impl Fn(NodeId) -> Option<Rc<Display>>,
) -> (Vec<Shape>, Vec<Part>) {
    let mut shapes = Vec::new();
    let mut open: Vec<(Opened, Vec<Part>)> = vec![(Opened::Root, Vec::new())];
    for item in items {
        let wanted: Vec<Opened> = Scope::chain(&item.scope)
            .into_iter()
            .map(|scope| Opened::Scope(scope.id, scope.group))
            .collect();
        let common = open[1..]
            .iter()
            .zip(&wanted)
            .take_while(|((opened, _), wanted)| opened == *wanted)
            .count();
        while open.len() > common + 1 {
            close(&mut open);
        }
        for opened in &wanted[common..] {
            open.push((*opened, Vec::new()));
        }
        let parts = &mut open.last_mut().expect("the root stays open").1;
        match item.recording {
            Recording::Shape(shape) => {
                let at = shapes.len() as u32;
                let bounds = crate::damage::bounds(&shape);
                shapes.push(shape);
                match parts.last_mut() {
                    Some(Part::Shapes {
                        end, bounds: held, ..
                    }) if *end == at => {
                        *end = at + 1;
                        *held = held.union(bounds);
                    }
                    _ => parts.push(Part::Shapes {
                        start: at,
                        end: at + 1,
                        bounds,
                    }),
                }
            }
            Recording::Child(id, at) => {
                if let Some(display) = child(id) {
                    parts.push(Part::Child(id, at, display));
                }
            }
        }
    }
    while open.len() > 1 {
        close(&mut open);
    }
    let (_, parts) = open.pop().expect("the root stays open");
    (shapes, parts)
}

#[derive(Clone, Copy, PartialEq)]
enum Opened {
    Root,
    Scope(u64, Group),
}

fn close(open: &mut Vec<(Opened, Vec<Part>)>) {
    let (opened, parts) = open.pop().expect("a group is open");
    let group = match opened {
        Opened::Scope(_, group) => group,
        Opened::Root => unreachable!("the root is never closed"),
    };
    if !parts.is_empty() {
        let into = &mut open.last_mut().expect("the root stays open").1;
        into.push(Part::Group(group, parts.into_boxed_slice()));
    }
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
    painting: &[(Rc<Display>, Vec2)],
    scroll: SpaceId,
    viewport: Rect,
    by: Vec2,
) -> (Region, Rect) {
    let mut fixed = Fixed {
        scroll,
        viewport,
        by,
        damage: Region::NOTHING,
        inner: viewport,
        faded: 0,
    };
    for (display, at) in painting {
        fixed.display(display, *at, Rect::EVERYTHING);
    }
    (fixed.damage, fixed.inner)
}

struct Fixed {
    scroll: SpaceId,
    viewport: Rect,
    by: Vec2,
    damage: Region,
    inner: Rect,
    faded: usize,
}

impl Fixed {
    fn display(&mut self, display: &Display, at: Vec2, clip: Rect) {
        if !display
            .bounds
            .translate(at)
            .intersect(clip)
            .intersects(self.viewport)
        {
            return;
        }
        self.parts(display, &display.parts, at, clip);
    }

    fn parts(&mut self, display: &Display, parts: &[Part], at: Vec2, clip: Rect) {
        for part in parts {
            match part {
                Part::Shapes { start, end, .. } => {
                    for shape in &display.shapes[*start as usize..*end as usize] {
                        self.shape(&placed_shape(shape, at, clip));
                    }
                }
                Part::Child(_, translation, child) => self.display(child, at + *translation, clip),
                Part::Group(group, inner) => match *group {
                    Group::Scroll { space, .. } if space == self.scroll => {}
                    Group::Scroll {
                        viewport,
                        offset,
                        scale: 1.0,
                        ..
                    } => self.parts(
                        display,
                        inner,
                        at + offset,
                        clip.intersect(viewport.translate(at)),
                    ),
                    Group::Clip(rect) => {
                        self.parts(display, inner, at, clip.intersect(rect.translate(at)))
                    }
                    Group::Transform { offset, scale: 1.0 } => {
                        self.parts(display, inner, at + offset, clip)
                    }
                    Group::Mask { fade, .. } => {
                        for band in fade.translate(at).bands() {
                            self.both(band.intersect(clip));
                        }
                        self.faded += 1;
                        self.parts(display, inner, at, clip);
                        self.faded -= 1;
                    }
                    Group::Scroll { .. } | Group::Transform { .. } | Group::Filter(_) => {
                        self.both(part.bounds().translate(at).intersect(clip));
                    }
                },
            }
        }
    }

    fn both(&mut self, rect: Rect) {
        self.damage.add(rect.intersect(self.viewport));
        self.damage
            .add(rect.translate(self.by).intersect(self.viewport));
    }

    fn shape(&mut self, shape: &Shape) {
        let bounds = crate::damage::bounds(shape);
        if !bounds.intersects(self.viewport) {
            return;
        }
        if covers(shape, self.viewport) {
            if self.faded == 0
                && matches!(shape, Shape::Rect { color, .. } if color.alpha() == u8::MAX)
            {
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
            self.both(rect);
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
            Some(rect.shrink(stroke_width + corner_radius.largest() + 1.0))
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
        && rect.shrink(corner_radius.largest()).contains_rect(viewport)
}

fn absolute(rect: Rect, state: PainterState) -> Rect {
    state.to_screen(rect.intersect(state.space_clip))
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

pub fn paint(doc: &Document, painter: &Painter, rects: &Rects, id: NodeId) {
    if doc.is_culled(id) {
        return;
    }
    let rect = rects.placed(&id).expect("node was not placed").rect;
    let ctx = painter.ctx();
    let own = painter.entered(id, rect);
    let at = rect.min.to_vec2();
    let placement = Placement {
        size: rect.size(),
        own: own.state(),
    };
    let visit = match doc.paint_cache.borrow_mut().visit(id, placement, at, rects) {
        Visit::Descend { .. } if doc.arena.get(id).turns_children() => Visit::Record,
        visit => visit,
    };
    match visit {
        Visit::Reuse => doc.note_painted(true),
        Visit::Descend { children, every } => {
            doc.note_painted(true);
            let visited: Vec<(NodeId, Option<Rc<Display>>)> = match every {
                true => Vec::new(),
                false => {
                    let cache = doc.paint_cache.borrow();
                    children
                        .iter()
                        .map(|(child, _)| (*child, cache.display(*child)))
                        .collect()
                }
            };
            ctx.measure_paint(|| {
                for (child, state) in children {
                    if rects.contains_key(&child) {
                        paint(doc, &Painter::resumed(ctx.clone(), state), rects, child);
                    }
                }
            });
            doc.paint_cache
                .borrow_mut()
                .settle_descended(id, &visited, every);
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
            doc.paint_cache
                .borrow_mut()
                .store(doc.zone(), id, placement, at, recorded);
        }
    }
    painter.paint_child(id, at);
}
