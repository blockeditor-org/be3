use std::any::Any;
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

use crate::base::list::Direction;
use crate::base::offset::item_rect;
use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::node::{Element, InteractInput, NodeId, NodeMap};
use crate::painter::Painter;
use crate::reactive::{
    KeyedItems, Prop, RenderFn, ScopeContext, create_effect, owner_scope, settle, with_document,
};
use beui_macros::component;

const ROW_LIMIT: usize = 4096;
const ANCHOR_SLACK: f32 = 4.0;

struct Row<K> {
    index: usize,
    key: K,
    node: NodeId,
    position: f32,
    length: f32,
}

struct Metrics {
    count: usize,
    estimated: f32,
    entries: Vec<(usize, f32)>,
    prefix: Vec<f32>,
    valid: usize,
    measured: f32,
}

impl Metrics {
    fn new(count: usize, estimated: f32) -> Self {
        Self::measured(count, estimated, Vec::new())
    }

    fn measured(count: usize, estimated: f32, entries: Vec<(usize, f32)>) -> Self {
        Self {
            count,
            estimated: estimated.max(0.0),
            prefix: vec![0.0; entries.len() + 1],
            valid: 0,
            measured: entries.iter().map(|(_, length)| length).sum(),
            entries,
        }
    }

    fn record(&mut self, index: usize, length: f32) {
        if index >= self.count {
            return;
        }
        let length = length.max(0.0);
        let at = self.entries.partition_point(|(held, _)| *held < index);
        match self.entries.get_mut(at) {
            Some((held, stored)) if *held == index => {
                if *stored == length {
                    return;
                }
                self.measured += length - *stored;
                *stored = length;
            }
            _ => {
                self.entries.insert(at, (index, length));
                self.prefix.push(0.0);
                self.measured += length;
            }
        }
        self.valid = self.valid.min(at);
    }

    fn height(&self, index: usize) -> f32 {
        let at = self.entries.partition_point(|(held, _)| *held < index);
        match self.entries.get(at) {
            Some((held, length)) if *held == index => *length,
            _ => self.estimated,
        }
    }

    fn measured_before(&mut self, at: usize) -> f32 {
        while self.valid < at {
            self.prefix[self.valid + 1] = self.prefix[self.valid] + self.entries[self.valid].1;
            self.valid += 1;
        }
        self.prefix[at]
    }

    fn position(&mut self, index: usize) -> f32 {
        let at = self.entries.partition_point(|(held, _)| *held < index);
        let measured = self.measured_before(at);
        (index - at) as f32 * self.estimated + measured
    }

    fn total(&self) -> f32 {
        (self.count - self.entries.len()) as f32 * self.estimated + self.measured
    }

    fn index_at(&mut self, position: f32) -> usize {
        if self.count == 0 {
            return 0;
        }
        let (mut low, mut high) = (0, self.count - 1);
        while low < high {
            let mid = low + (high - low).div_ceil(2);
            match self.position(mid) <= position {
                true => low = mid,
                false => high = mid - 1,
            }
        }
        low
    }
}

pub(crate) struct VirtualListNode<K> {
    pub(crate) direction: Direction,
    keys: Vec<K>,
    indices: HashMap<K, usize>,
    sizes: HashMap<K, f32>,
    metrics: Metrics,
    rows: Rc<KeyedItems<K, NodeId>>,
    owner: Option<ScopeContext>,
    placed: Vec<Row<K>>,
    origin: Option<(K, f32)>,
}

impl<K: Clone + Hash + Eq + 'static> VirtualListNode<K> {
    fn new(build: impl Fn(K) -> NodeId + 'static) -> Self {
        Self {
            direction: Direction::Vertical,
            keys: Vec::new(),
            indices: HashMap::new(),
            sizes: HashMap::new(),
            metrics: Metrics::new(0, 0.0),
            rows: Rc::new(KeyedItems::new(build)),
            owner: owner_scope(),
            placed: Vec::new(),
            origin: None,
        }
    }

    fn set_keys(&mut self, keys: Vec<K>) -> Vec<NodeId> {
        let indices: HashMap<K, usize> = keys
            .iter()
            .enumerate()
            .map(|(index, key)| (key.clone(), index))
            .collect();
        assert_eq!(
            indices.len(),
            keys.len(),
            "a virtual list was given the same key twice"
        );
        self.sizes.retain(|key, _| indices.contains_key(key));
        let mut entries: Vec<(usize, f32)> = self
            .sizes
            .iter()
            .map(|(key, length)| (indices[key], *length))
            .collect();
        entries.sort_unstable_by_key(|(index, _)| *index);
        self.metrics = Metrics::measured(keys.len(), self.metrics.estimated, entries);
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
        for row in &mut self.placed {
            row.index = indices[&row.key];
        }
        self.keys = keys;
        self.indices = indices;
        let kept: Vec<K> = self.placed.iter().map(|row| row.key.clone()).collect();
        self.rows.retain(&kept)
    }

    fn window(&self, doc: &Document, painter: &Painter, start: f32, main: f32) -> (f32, f32) {
        let clip = painter.clip_rect().intersect(doc.viewport_rect());
        let (from, to) = match self.direction {
            Direction::Horizontal => (clip.left(), clip.right()),
            Direction::Vertical => (clip.top(), clip.bottom()),
        };
        let first = (from - start).max(0.0);
        let last = (to - start).min(main).max(first);
        (first, last)
    }

    fn build(&self, key: K) -> NodeId {
        if let Some(node) = self.rows.get(&key) {
            return node;
        }
        let rows = self.rows.clone();
        match self.owner.as_ref().filter(|owner| owner.is_alive()) {
            Some(owner) => settle(|| owner.run(|| rows.entry(key))),
            None => settle(|| rows.entry(key)),
        }
    }

    fn measured_row(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        index: usize,
        cross: f32,
    ) -> (K, NodeId, f32) {
        let key = self.keys[index].clone();
        let node = self.build(key.clone());
        let offer = self.direction.axes(f32::INFINITY, cross);
        let length = self
            .direction
            .main(crate::layout::measure(doc, painter, node, offer));
        self.metrics.record(index, length);
        self.sizes.insert(key.clone(), length);
        (key, node, length)
    }

    fn anchor(&mut self, window: (f32, f32)) -> (usize, f32) {
        let (first, last) = window;
        let slack = (last - first) + ANCHOR_SLACK * self.metrics.estimated.max(1.0);
        if let Some((key, position)) = &self.origin
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
        painter: &Painter,
        window: (f32, f32),
        main: f32,
        cross: f32,
    ) -> f32 {
        let (first, last) = window;
        if self.metrics.count == 0 || last <= first {
            self.origin = None;
            self.keep(doc, Vec::new());
            return 0.0;
        }
        let (index, position) = self.anchor(window);
        let mut below = Vec::new();
        let (mut cursor, mut at, mut scanned) = (position, index, 0);
        while at < self.metrics.count && cursor < last && scanned < ROW_LIMIT {
            let estimate = self.metrics.height(at);
            let row = match cursor + estimate > first {
                true => Some(self.measured_row(doc, painter, at, cross)),
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
        let mut above = Vec::new();
        let (mut cursor, mut at, mut scanned) = (position, index, 0);
        while at > 0 && cursor > first && scanned < ROW_LIMIT {
            at -= 1;
            scanned += 1;
            let estimate = self.metrics.height(at);
            let row = match cursor - estimate < last {
                true => Some(self.measured_row(doc, painter, at, cross)),
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
        let shifted = self.edge_shift(&rows, window, main);
        if shifted != 0.0 {
            for row in &mut rows {
                row.position += shifted;
            }
        }
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
        self.keep(doc, rows);
        shift
    }

    fn edge_shift(&self, rows: &[Row<K>], window: (f32, f32), main: f32) -> f32 {
        let (Some(head), Some(tail)) = (rows.first(), rows.last()) else {
            return 0.0;
        };
        let (first, last) = window;
        if head.index == 0 && first <= 0.0 {
            return -head.position;
        }
        if tail.index + 1 == self.metrics.count && last >= main {
            return main - (tail.position + tail.length);
        }
        0.0
    }

    fn keep(&mut self, doc: &mut Document, rows: Vec<Row<K>>) {
        let kept: Vec<K> = rows.iter().map(|row| row.key.clone()).collect();
        self.placed = rows;
        for node in self.rows.retain(&kept) {
            doc.remove_node(node);
        }
    }
}

impl<K: Clone + Hash + Eq + 'static> Element for VirtualListNode<K> {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        let (_, cross) = self.direction.main_and_cross(available);
        let offer = self.direction.axes(f32::INFINITY, cross);
        let content = self
            .placed
            .iter()
            .map(|row| {
                let size = crate::layout::measure(doc, painter, row.node, offer);
                self.direction.main_and_cross(size).1
            })
            .fold(0.0_f32, f32::max);
        self.direction.axes(self.metrics.total(), content)
    }

    fn layout(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        rect: Rect,
        out: &mut NodeMap<Rect>,
    ) {
        let node = doc.laying_out();
        let (main, cross) = self.direction.main_and_cross(rect.size());
        let start = self.direction.main(rect.min.to_vec2());
        let live = doc.delivering();
        let total = self.metrics.total();
        let shift = match live {
            true => {
                let window = self.window(doc, painter, start, main);
                self.realize(doc, painter, window, main, cross)
            }
            false => 0.0,
        };
        let grid = doc.pixel_grid();
        for row in &self.placed {
            let placed = item_rect(
                self.direction,
                rect,
                start + grid.snap(row.position),
                row.length,
            );
            crate::layout::layout(doc, painter, row.node, placed, out);
        }
        if !live {
            return;
        }
        if let Some(node) = node
            && self.metrics.total() != total
        {
            doc.invalidate_measurement(node);
        }
        if shift != 0.0 {
            doc.record_scroll_shift(shift);
        }
    }

    fn unplaced(&mut self, doc: &mut Document) {
        self.origin = None;
        self.keep(doc, Vec::new());
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &NodeMap<Rect>, _rect: Rect) {
        for row in &self.placed {
            if rects.contains_key(&row.node) {
                crate::paint::paint(doc, painter, rects, row.node);
            }
        }
    }

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _input: &InteractInput,
        _id: NodeId,
        _rect: Rect,
        _focus_target: &mut Option<NodeId>,
        children: &mut Vec<NodeId>,
    ) {
        children.extend(self.placed.iter().map(|row| row.node));
    }

    fn children(&self) -> Vec<NodeId> {
        self.placed.iter().map(|row| row.node).collect()
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
                self.metrics.count
            )
        });
        let detail = [horizontal.map(str::to_string), realized]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        (!detail.is_empty()).then_some(detail)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub(crate) fn create_virtual_list<K: Clone + Hash + Eq + 'static>(
        &mut self,
        build: impl Fn(K) -> NodeId + 'static,
    ) -> NodeId {
        self.arena.insert(VirtualListNode::new(build))
    }

    pub(crate) fn set_virtual_list_direction<K: Clone + Hash + Eq + 'static>(
        &mut self,
        list: NodeId,
        direction: Direction,
    ) {
        if self.arena.get_as::<VirtualListNode<K>>(list).direction == direction {
            return;
        }
        let node = self.arena.get_mut_as::<VirtualListNode<K>>(list);
        node.direction = direction;
        node.origin = None;
    }

    pub(crate) fn set_virtual_list_keys<K: Clone + Hash + Eq + 'static>(
        &mut self,
        list: NodeId,
        keys: Vec<K>,
    ) {
        if self.arena.get_as::<VirtualListNode<K>>(list).keys == keys {
            return;
        }
        let evicted = self
            .arena
            .get_mut_as::<VirtualListNode<K>>(list)
            .set_keys(keys);
        for row in evicted {
            self.remove_node(row);
        }
    }

    pub(crate) fn set_virtual_list_item_size<K: Clone + Hash + Eq + 'static>(
        &mut self,
        list: NodeId,
        item_size: f32,
    ) {
        let item_size = item_size.max(0.0);
        if self
            .arena
            .get_as::<VirtualListNode<K>>(list)
            .metrics
            .estimated
            == item_size
        {
            return;
        }
        let node = self.arena.get_mut_as::<VirtualListNode<K>>(list);
        node.sizes.clear();
        node.metrics = Metrics::new(node.metrics.count, item_size);
    }
}

#[component]
pub fn VirtualList<K>(
    keys: Prop<Vec<K>>,
    item_size: Prop<f32>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(children)] item: RenderFn<K>,
) -> NodeId
where
    K: Clone + Hash + Eq + 'static,
{
    let list = with_document(|document| document.create_virtual_list(move |key| item.call(key)));
    create_effect(move || {
        with_document(|document| document.set_virtual_list_direction::<K>(list, direction.get()))
    });
    create_effect(move || {
        let keys = keys.get();
        with_document(|document| document.set_virtual_list_keys(list, keys));
    });
    create_effect(move || {
        with_document(|document| document.set_virtual_list_item_size::<K>(list, item_size.get()))
    });
    list
}
