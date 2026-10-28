use foldhash::{HashMap, HashSet};
use std::any::Any;
use std::collections::VecDeque;
use std::hash::Hash;
use std::rc::Rc;

use crate::base::list::Direction;
use crate::base::offset::item_rect;
use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::node::{Element, FocusChild, NodeId, NodeOf, Rects};
use crate::painter::Painter;
use ::reactive::{KeyedItems, ScopeContext, numbered, owner_scope};
use beui_types::rows::RowMetrics;

const ROW_LIMIT: usize = 4096;
const ANCHOR_SLACK: f32 = 4.0;

#[derive(Clone)]
struct Row<K> {
    index: usize,
    key: K,
    node: NodeId,
    position: f32,
    length: f32,
}

pub struct VirtualListNode<K> {
    pub direction: Direction,
    keys: Vec<K>,
    indices: HashMap<K, usize>,
    sizes: HashMap<K, f32>,
    metrics: RowMetrics,
    rows: Rc<KeyedItems<K, NodeId>>,
    owner: Option<ScopeContext>,
    placed: Vec<Row<K>>,
    held: Vec<Row<K>>,
    retained: VecDeque<K>,
    origin: Option<(K, f32)>,
    overscan: f32,
}

impl<K: Clone + Hash + Eq + 'static> VirtualListNode<K> {
    fn new(build: impl Fn(K) -> NodeId + 'static) -> Self {
        Self {
            direction: Direction::Vertical,
            keys: Vec::new(),
            indices: HashMap::default(),
            sizes: HashMap::default(),
            metrics: RowMetrics::new(0, 0.0),
            rows: Rc::new(KeyedItems::new(build)),
            owner: owner_scope(),
            placed: Vec::new(),
            held: Vec::new(),
            retained: VecDeque::new(),
            origin: None,
            overscan: 0.0,
        }
    }

    fn set_keys(&mut self, keys: Vec<K>) {
        let indices: HashMap<K, usize> = keys
            .iter()
            .enumerate()
            .map(|(index, key)| (key.clone(), index))
            .collect();
        self.sizes.retain(|key, _| indices.contains_key(key));
        let mut entries: Vec<(usize, f32)> = self
            .sizes
            .iter()
            .map(|(key, length)| (indices[key], *length))
            .collect();
        entries.sort_unstable_by_key(|(index, _)| *index);
        self.metrics = RowMetrics::measured(keys.len(), self.metrics.estimated(), entries);
        self.origin = self.origin.take().and_then(|(key, position)| {
            if indices.contains_key(&key) {
                return Some((key, position));
            }
            let head = self.placed.first()?.position;
            self.placed
                .iter()
                .find(|row| indices.contains_key(&row.key))
                .map(|row| (row.key.clone(), position + row.position - head))
        });
        self.placed.retain(|row| indices.contains_key(&row.key));
        self.held.retain(|row| indices.contains_key(&row.key));
        for row in self.placed.iter_mut().chain(&mut self.held) {
            row.index = indices[&row.key];
        }
        self.held.sort_unstable_by_key(|row| row.index);
        self.retained.retain(|key| indices.contains_key(key));
        self.keys = keys;
        self.indices = indices;
        self.release_unkept();
    }

    fn release_unkept(&self) {
        let kept: Vec<K> = self
            .placed
            .iter()
            .chain(&self.held)
            .map(|row| row.key.clone())
            .chain(self.retained.iter().cloned())
            .collect();
        self.rows.retain(&kept);
    }

    fn shown(&self) -> impl Iterator<Item = &Row<K>> {
        let split = self.placed.first().map_or(self.held.len(), |head| {
            self.held.partition_point(|row| row.index < head.index)
        });
        self.held[..split]
            .iter()
            .chain(&self.placed)
            .chain(&self.held[split..])
    }

    fn holding(&self, doc: &Document, rows: &[Row<K>]) -> Vec<Row<K>> {
        let shown: HashSet<&K> = rows.iter().map(|row| &row.key).collect();
        let mut held: Vec<Row<K>> = self
            .placed
            .iter()
            .chain(&self.held)
            .filter(|row| !shown.contains(&row.key) && doc.focus_is_within(row.node))
            .cloned()
            .collect();
        held.sort_unstable_by_key(|row| row.index);
        held
    }

    fn measure_held(
        &mut self,
        doc: &mut Document,
        list: NodeId,
        painter: &Painter,
        held: &mut [Row<K>],
        cross: f32,
    ) {
        for row in held {
            row.length = self.measured_row(doc, list, painter, row.index, cross).2;
        }
    }

    fn window(&self, doc: &Document, painter: &Painter, start: f32, main: f32) -> (f32, f32) {
        let state = painter.state();
        let clip = state
            .clip
            .intersect(state.space_clip)
            .intersect(state.to_local(doc.viewport_rect()));
        let (from, to) = match self.direction {
            Direction::Horizontal => (clip.left(), clip.right()),
            Direction::Vertical => (clip.top(), clip.bottom()),
        };
        let first = (from - start).max(0.0);
        let last = (to - start).min(main).max(first);
        (first, last)
    }

    fn build(&self, doc: &mut Document, list: NodeId, key: K) -> NodeId {
        if let Some(node) = self.rows.get(&key) {
            return node;
        }
        let rows = self.rows.clone();
        let owner = self.owner.clone().filter(ScopeContext::is_alive);
        doc.build_within(list, move || match owner {
            Some(owner) => owner.run(|| rows.entry_owned(key)),
            None => rows.entry_owned(key),
        })
    }

    fn measured_row(
        &mut self,
        doc: &mut Document,
        list: NodeId,
        painter: &Painter,
        index: usize,
        cross: f32,
    ) -> (K, NodeId, f32) {
        let key = self.keys[index].clone();
        let node = self.build(doc, list, key.clone());
        let offer = self.direction.axes(f32::INFINITY, cross);
        let length = self
            .direction
            .main(crate::layout::measure(doc, painter, node, offer));
        self.metrics.record(index, length);
        self.sizes.insert(key.clone(), length);
        (key, node, length)
    }

    fn anchor(&mut self, window: (f32, f32), at_start: bool) -> (usize, f32) {
        let (first, last) = window;
        let slack = (last - first) + ANCHOR_SLACK * self.metrics.estimated().max(1.0);
        if !at_start
            && let Some((key, position)) = &self.origin
            && let Some(&index) = self.indices.get(key)
            && *position >= first - slack
            && *position <= last + slack
        {
            return (index, *position);
        }
        let index = self.metrics.index_at(first);
        (index, self.metrics.position(index))
    }

    fn realize(
        &mut self,
        doc: &mut Document,
        list: NodeId,
        painter: &Painter,
        window: (f32, f32),
        main: Option<f32>,
        cross: f32,
    ) -> f32 {
        if self.metrics.count() == 0 || window.1 <= window.0 {
            self.origin = None;
            let mut held = self.holding(doc, &[]);
            self.measure_held(doc, list, painter, &mut held, cross);
            for row in &mut held {
                row.position = self.metrics.position(row.index);
            }
            self.keep(Vec::new(), held);
            return 0.0;
        }
        let at_start = window.0 <= 0.0;
        let window = (
            (window.0 - self.overscan).max(0.0),
            (window.1 + self.overscan).min(main.unwrap_or(f32::INFINITY)),
        );
        let pinned = doc.pinned_to_end() == Some(self.direction);
        let before = self.metrics.total();
        let rows = match pinned {
            true => self.rows_from_end(doc, list, painter, window, main.unwrap_or(before), cross),
            false => self.rows_from_anchor(doc, list, painter, window, main, cross, at_start),
        };
        let mut held = self.holding(doc, &rows);
        self.measure_held(doc, list, painter, &mut held, cross);
        let shift = match rows.first() {
            Some(row) => {
                let wanted = self.metrics.position(row.index);
                let shift = wanted - row.position;
                self.origin = Some((row.key.clone(), wanted));
                shift
            }
            None => {
                self.origin = None;
                0.0
            }
        };
        for row in &mut held {
            row.position = self.metrics.position(row.index) - shift;
        }
        self.keep(rows, held);
        shift
    }

    fn rows_from_end(
        &mut self,
        doc: &mut Document,
        list: NodeId,
        painter: &Painter,
        window: (f32, f32),
        end: f32,
        cross: f32,
    ) -> Vec<Row<K>> {
        let (first, last) = window;
        let mut rows = Vec::new();
        let (mut cursor, mut at, mut scanned) = (end, self.metrics.count(), 0);
        while at > 0 && cursor > first && scanned < ROW_LIMIT {
            at -= 1;
            scanned += 1;
            let (key, node, length) = self.measured_row(doc, list, painter, at, cross);
            cursor -= length;
            if cursor < last {
                rows.push(Row {
                    index: at,
                    key,
                    node,
                    position: cursor,
                    length,
                });
            }
        }
        rows.reverse();
        rows
    }

    #[allow(clippy::too_many_arguments)]
    fn rows_from_anchor(
        &mut self,
        doc: &mut Document,
        list: NodeId,
        painter: &Painter,
        window: (f32, f32),
        main: Option<f32>,
        cross: f32,
        at_start: bool,
    ) -> Vec<Row<K>> {
        let (first, last) = window;
        let (index, position) = self.anchor(window, at_start);
        let before = self.metrics.total();
        let mut below = Vec::new();
        let (mut cursor, mut at, mut scanned) = (position, index, 0);
        while at < self.metrics.count() && cursor < self.reach(last, main) && scanned < ROW_LIMIT {
            let estimate = self.metrics.height(at);
            let row = match cursor + estimate > first {
                true => Some(self.measured_row(doc, list, painter, at, cross)),
                false => None,
            };
            let length = row.as_ref().map_or(estimate, |(_, _, length)| *length);
            if let Some((key, node, _)) = row
                && cursor + length > first
            {
                below.push(Row {
                    index: at,
                    key,
                    node,
                    position: cursor,
                    length,
                });
            }
            cursor += length;
            at += 1;
            scanned += 1;
        }
        let shrunk = (before - self.metrics.total()).max(0.0);
        let deficit = match at >= self.metrics.count() {
            true => (last - cursor).clamp(0.0, shrunk),
            false => 0.0,
        };
        let mut above = Vec::new();
        let (mut cursor, mut at, mut scanned) = (position, index, 0);
        while at > 0 && cursor > first - deficit && scanned < ROW_LIMIT {
            at -= 1;
            scanned += 1;
            let estimate = self.metrics.height(at);
            let row = match cursor - estimate < last {
                true => Some(self.measured_row(doc, list, painter, at, cross)),
                false => None,
            };
            let length = row.as_ref().map_or(estimate, |(_, _, length)| *length);
            cursor -= length;
            if let Some((key, node, _)) = row
                && cursor < last
            {
                above.push(Row {
                    index: at,
                    key,
                    node,
                    position: cursor,
                    length,
                });
            }
        }
        above.reverse();
        above.extend(below);
        let mut rows = above;
        let end = self.metrics.total();
        let main = main.map_or(end, |main| main.min(end));
        let shifted = self.edge_shift(&rows, (first, last.min(main)), main);
        if shifted != 0.0 {
            for row in &mut rows {
                row.position += shifted;
            }
        }
        rows
    }

    fn reach(&self, last: f32, main: Option<f32>) -> f32 {
        match main {
            Some(_) => last,
            None => last.min(self.metrics.total()),
        }
    }

    fn edge_shift(&self, rows: &[Row<K>], window: (f32, f32), main: f32) -> f32 {
        let (Some(head), Some(tail)) = (rows.first(), rows.last()) else {
            return 0.0;
        };
        let (first, last) = window;
        if head.index == 0 && first <= 0.0 {
            return -head.position;
        }
        if tail.index + 1 == self.metrics.count() && last >= main {
            return main - (tail.position + tail.length);
        }
        0.0
    }

    fn keep(&mut self, rows: Vec<Row<K>>, held: Vec<Row<K>>) {
        let placed: HashSet<K> = rows
            .iter()
            .chain(&held)
            .map(|row| row.key.clone())
            .collect();
        for row in self.placed.iter().chain(&self.held).rev() {
            if !placed.contains(&row.key) {
                self.retained.push_front(row.key.clone());
            }
        }
        let reach = rows.len();
        let near = match (rows.first(), rows.last()) {
            (Some(first), Some(last)) => {
                first.index.saturating_sub(reach)..last.index.saturating_add(reach + 1)
            }
            _ => 0..0,
        };
        let indices = &self.indices;
        self.retained.retain(|key| {
            !placed.contains(key) && indices.get(key).is_some_and(|index| near.contains(index))
        });
        self.retained.truncate(reach);
        self.placed = rows;
        self.held = held;
        self.release_unkept();
    }
}

impl<K: Clone + Hash + Eq + 'static> Element for VirtualListNode<K> {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        let (_, cross) = self.direction.main_and_cross(available);
        let offer = self.direction.axes(f32::INFINITY, cross);
        let content = self
            .shown()
            .map(|row| {
                let size = crate::layout::measure(doc, painter, row.node, offer);
                self.direction.main_and_cross(size).1
            })
            .fold(0.0_f32, f32::max);
        self.direction.axes(self.metrics.total(), content)
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        let Some(node) = doc.laying_out() else {
            return;
        };
        let (main, cross) = self.direction.main_and_cross(rect.size());
        let start = self.direction.main(rect.min.to_vec2());
        let total = self.metrics.total();
        let main = (doc.pixel_grid().snap(total) != main).then_some(main);
        let window = self.window(doc, painter, start, main.unwrap_or(f32::INFINITY));
        let shift = self.realize(doc, node, painter, window, main, cross);
        if window.0 > 0.0 || window.1 < self.metrics.total() {
            painter.ctx().note_space_read();
        }
        let grid = doc.pixel_grid();
        for row in self.shown() {
            let placed = item_rect(
                self.direction,
                rect,
                start + grid.snap(row.position),
                row.length,
            );
            crate::layout::layout(doc, painter, row.node, placed, out);
        }
        doc.arena.adopt_children(node);
        if self.metrics.total() != total {
            doc.invalidate_after_layout(node);
        }
        doc.report_anchor_shift(shift);
    }

    fn unplaced(&mut self, doc: &mut Document) {
        self.origin = None;
        let held = self.holding(doc, &[]);
        self.keep(Vec::new(), held);
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, _rect: Rect) {
        let shown = || {
            self.shown()
                .map(|row| row.node)
                .filter(|node| rects.contains_key(node))
        };
        crate::paint::paint_raised_last(doc, painter, rects, shown);
    }

    fn children(&self) -> Vec<NodeId> {
        self.shown().map(|row| row.node).collect()
    }

    fn remove_child(&mut self, child: NodeId) {
        self.placed.retain(|row| row.node != child);
        self.held.retain(|row| row.node != child);
    }

    fn focus_children(&self) -> Option<Vec<FocusChild>> {
        let mut rows: Vec<&Row<K>> = self.shown().collect();
        rows.sort_unstable_by_key(|row| row.index);
        let mut children = Vec::with_capacity(rows.len() * 2 + 1);
        let mut next = 0;
        for row in rows {
            if row.index > next {
                children.push(FocusChild::Unbuilt(next..row.index));
            }
            children.push(FocusChild::Node(row.node));
            next = row.index + 1;
        }
        if next < self.metrics.count() {
            children.push(FocusChild::Unbuilt(next..self.metrics.count()));
        }
        Some(children)
    }

    fn build_for_focus(&mut self, doc: &mut Document, id: NodeId, index: usize) -> Option<NodeId> {
        if let Some(row) = self.shown().find(|row| row.index == index) {
            return Some(row.node);
        }
        let key = self.keys.get(index)?.clone();
        let node = self.build(doc, id, key.clone());
        let drift = self.placed.first().map_or(0.0, |head| {
            head.position - self.metrics.position(head.index)
        });
        let row = Row {
            index,
            key,
            node,
            position: self.metrics.position(index) + drift,
            length: self.metrics.height(index),
        };
        let at = self.held.partition_point(|held| held.index < index);
        self.held.insert(at, row);
        Some(node)
    }

    fn release_for_focus(&mut self, child: NodeId) {
        let before = self.held.len();
        self.held.retain(|row| row.node != child);
        if self.held.len() != before {
            self.release_unkept();
        }
    }

    fn kind(&self) -> &'static str {
        "virtual list"
    }

    fn detail(&self) -> Option<String> {
        let horizontal = (self.direction == Direction::Horizontal).then_some("horizontal");
        let realized = self.placed.first().map(|row| {
            format!(
                "{}..{} of {}",
                row.index,
                row.index + self.placed.len(),
                self.metrics.count()
            )
        });
        let detail = [horizontal.map(str::to_string), realized]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        (!detail.is_empty()).then_some(detail)
    }

    fn properties(&self) -> Vec<(&'static str, String)> {
        vec![
            ("direction", format!("{:?}", self.direction)),
            ("rows", self.metrics.count().to_string()),
            ("realized", self.placed.len().to_string()),
            ("held for focus", self.held.len().to_string()),
            ("estimated row length", self.metrics.estimated().to_string()),
        ]
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_virtual_list<K: Clone + Hash + Eq + 'static>(
        &mut self,
        build: impl Fn(K) -> NodeId + 'static,
    ) -> NodeOf<VirtualListNode<(K, usize)>> {
        self.arena
            .insert(VirtualListNode::new(move |(key, _): (K, usize)| build(key)))
    }

    pub fn set_virtual_list_direction<K: Clone + Hash + Eq + 'static>(
        &mut self,
        list: NodeOf<VirtualListNode<(K, usize)>>,
        direction: Direction,
    ) {
        if self
            .arena
            .get_as::<VirtualListNode<(K, usize)>>(list)
            .direction
            == direction
        {
            return;
        }
        let node = self.arena.get_mut_as::<VirtualListNode<(K, usize)>>(list);
        node.direction = direction;
        node.origin = None;
    }

    pub fn set_virtual_list_keys<K: Clone + Hash + Eq + 'static>(
        &mut self,
        list: NodeOf<VirtualListNode<(K, usize)>>,
        keys: Vec<K>,
    ) {
        let keys = numbered(keys);
        if self.arena.get_as::<VirtualListNode<(K, usize)>>(list).keys == keys {
            return;
        }
        self.arena
            .get_mut_as::<VirtualListNode<(K, usize)>>(list)
            .set_keys(keys);
    }

    pub fn virtual_list_offset<K: Clone + Hash + Eq + 'static>(
        &self,
        list: NodeId,
        key: &K,
    ) -> Option<f32> {
        let list = self.arena.kind_of::<VirtualListNode<(K, usize)>>(list)?;
        let node = self.arena.get_as(list);
        let index = *node.indices.get(&(key.clone(), 0))?;
        Some(node.metrics.position(index))
    }

    pub fn set_virtual_list_overscan<K: Clone + Hash + Eq + 'static>(
        &mut self,
        list: NodeOf<VirtualListNode<(K, usize)>>,
        overscan: f32,
    ) {
        let overscan = overscan.max(0.0);
        if self
            .arena
            .get_as::<VirtualListNode<(K, usize)>>(list)
            .overscan
            == overscan
        {
            return;
        }
        self.arena
            .get_mut_as::<VirtualListNode<(K, usize)>>(list)
            .overscan = overscan;
    }

    pub fn set_virtual_list_item_size<K: Clone + Hash + Eq + 'static>(
        &mut self,
        list: NodeOf<VirtualListNode<(K, usize)>>,
        item_size: f32,
    ) {
        let item_size = item_size.max(0.0);
        if self
            .arena
            .get_as::<VirtualListNode<(K, usize)>>(list)
            .metrics
            .estimated()
            == item_size
        {
            return;
        }
        let node = self.arena.get_mut_as::<VirtualListNode<(K, usize)>>(list);
        node.sizes.clear();
        node.metrics = RowMetrics::new(node.metrics.count(), item_size);
    }
}
