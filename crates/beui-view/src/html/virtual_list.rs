use stable_hash::HashMap;
use std::cell::{Cell, RefCell};
use std::hash::Hash;
use std::rc::Rc;

use crate::reactive::Callback;
use crate::reactive::Get;
use crate::reactive::KeyedItems;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::ReadSignal;
use crate::reactive::RenderFn;
use crate::reactive::ScopeContext;
use crate::reactive::SlotHost;
use crate::reactive::SlotId;
use crate::reactive::create_binding;
use crate::reactive::create_signal;
use crate::reactive::on_cleanup;
use crate::reactive::owner_scope;
use crate::reactive::untrack;
use crate::types::Direction;
use crate::types::rows::RowMetrics;
use beui_core_html::css;
use beui_core_html::document::{create, set_style};
use beui_core_html::rows::{Watch, main_length, spacer, visible_span};
use reactive::numbered;

const ROW_LIMIT: usize = 4096;
const FIRST_REACH: f32 = 1024.0;
const FLING_FRAMES: f32 = 6.0;

struct Rows<K: Clone + Hash + Eq + 'static> {
    list: NodeId,
    lead: NodeId,
    trail: NodeId,
    host: Rc<dyn SlotHost<NodeId>>,
    slot: SlotId,
    owner: Option<ScopeContext>,
    direction: Cell<Direction>,
    keys: RefCell<Vec<(K, usize)>>,
    sizes: RefCell<HashMap<(K, usize), f32>>,
    metrics: RefCell<RowMetrics>,
    items: KeyedItems<(K, usize), NodeId>,
    shown: Cell<(usize, usize)>,
    nodes: RefCell<Vec<NodeId>>,
    overscan: Cell<f32>,
    first: Cell<Option<f32>>,
    stale: Cell<bool>,
    watch: Watch,
}

impl<K: Clone + Hash + Eq + 'static> Rows<K> {
    fn set_keys(&self, keys: Vec<K>) {
        let keys = numbered(keys);
        if *self.keys.borrow() == keys {
            return;
        }
        let entries = {
            let mut sizes = self.sizes.borrow_mut();
            let indices: HashMap<&(K, usize), usize> =
                keys.iter().enumerate().map(|(at, key)| (key, at)).collect();
            sizes.retain(|key, _| indices.contains_key(key));
            let mut entries: Vec<(usize, f32)> = sizes
                .iter()
                .map(|(key, length)| (indices[key], *length))
                .collect();
            entries.sort_unstable_by_key(|(at, _)| *at);
            entries
        };
        let estimated = self.metrics.borrow().estimated();
        *self.metrics.borrow_mut() = RowMetrics::measured(keys.len(), estimated, entries);
        *self.keys.borrow_mut() = keys;
        self.stale.set(true);
        beui_core_html::frame::request_frame();
    }

    fn set_estimate(&self, estimated: f32) {
        let estimated = estimated.max(0.0);
        if self.metrics.borrow().estimated() == estimated {
            return;
        }
        self.sizes.borrow_mut().clear();
        let count = self.keys.borrow().len();
        *self.metrics.borrow_mut() = RowMetrics::new(count, estimated);
        self.stale.set(true);
        beui_core_html::frame::request_frame();
    }

    fn set_direction(&self, direction: Direction) {
        self.direction.set(direction);
        beui_core_html::document::set_direction(self.list, direction);
        set_style(
            self.list,
            "flex-direction",
            Some(css::flex_direction(direction).to_owned()),
        );
        self.first.set(None);
        self.stale.set(true);
        beui_core_html::frame::request_frame();
    }

    fn range(&self, from: f32, to: f32) -> (usize, usize) {
        let metrics = self.metrics.borrow();
        let count = metrics.count();
        if count == 0 || to <= from {
            return (0, 0);
        }
        let start = metrics.index_at(from);
        let (mut end, mut cursor) = (start, metrics.position(start));
        while end < count && cursor < to && end - start < ROW_LIMIT {
            cursor += metrics.height(end);
            end += 1;
        }
        (start, end)
    }

    fn build(&self, range: (usize, usize)) {
        let keys = self.keys.borrow()[range.0..range.1].to_vec();
        let mapping = match &self.owner {
            Some(owner) if owner.is_alive() => owner.run(|| self.items.map(keys)),
            _ => self.items.map(keys),
        };
        let nodes = mapping.items().to_vec();
        for node in self.nodes.borrow().iter() {
            if !nodes.contains(node) {
                self.watch.unobserve(*node);
            }
        }
        let host = Rc::clone(&self.host);
        let slot = self.slot;
        mapping.commit(|items, _| host.fill(slot, items));
        for node in &nodes {
            self.watch.observe(*node);
        }
        *self.nodes.borrow_mut() = nodes;
        self.shown.set(range);
    }

    fn place_spacers(&self) {
        let (start, end) = self.shown.get();
        let metrics = self.metrics.borrow();
        let direction = self.direction.get();
        spacer(direction, self.lead, metrics.position(start));
        spacer(
            direction,
            self.trail,
            metrics.total() - metrics.position(end),
        );
    }

    fn measure(&self) -> bool {
        let direction = self.direction.get();
        let (start, _) = self.shown.get();
        let keys = self.keys.borrow();
        let mut metrics = self.metrics.borrow_mut();
        let mut sizes = self.sizes.borrow_mut();
        let mut changed = false;
        for (at, node) in self.nodes.borrow().iter().enumerate() {
            let index = start + at;
            let (Some(key), Some(length)) = (keys.get(index), main_length(*node, direction)) else {
                continue;
            };
            if metrics.record(index, length) {
                sizes.insert(key.clone(), length);
                changed = true;
            }
        }
        changed
    }

    fn lay_out(&self) -> bool {
        let mut changed = self.measure();
        let window = visible_span(self.list, self.direction.get());
        let range = match window {
            Some((first, last)) => {
                let motion = self
                    .first
                    .replace(Some(first))
                    .map_or(0.0, |was| first - was);
                let ahead = (motion.abs() * FLING_FRAMES).min((last - first) * 2.0);
                let overscan = self.overscan.get();
                let behind = if motion < 0.0 { ahead } else { 0.0 };
                let beyond = if motion > 0.0 { ahead } else { 0.0 };
                self.range(
                    (first - overscan - behind).max(0.0),
                    last + overscan + beyond,
                )
            }
            None if self.stale.get() => self.range(0.0, FIRST_REACH),
            None => self.shown.get(),
        };
        if self.stale.replace(false) || range != self.shown.get() {
            self.build(range);
            changed = true;
        }
        if changed {
            self.place_spacers();
        }
        changed
    }
}

pub(crate) fn virtual_list<K>(
    keys: Prop<Vec<K>>,
    item_size: Prop<f32>,
    direction: Prop<Direction>,
    overscan: Prop<f32>,
    text: Callback<K, String>,
    item: RenderFn<ReadSignal<K>>,
) -> NodeId
where
    K: Clone + Hash + Eq + 'static,
{
    let _ = text;
    let list = create("VirtualList", "div");
    set_style(list, "display", Some("flex".to_owned()));
    let lead = create("VirtualListSpacer", "div");
    let trail = create("VirtualListSpacer", "div");
    let host = beui_core_html::tree::node_host(list);
    host.append(lead);
    let slot = host.open();
    host.append(trail);
    let rows = Rc::new(Rows {
        list,
        lead,
        trail,
        host,
        slot,
        owner: owner_scope(),
        direction: Cell::new(Direction::Vertical),
        keys: RefCell::default(),
        sizes: RefCell::default(),
        metrics: RefCell::new(RowMetrics::new(0, 0.0)),
        items: KeyedItems::new(move |(key, _): (K, usize)| {
            item.call(create_signal(key).read_only())
        }),
        shown: Cell::new((0, 0)),
        nodes: RefCell::default(),
        overscan: Cell::new(0.0),
        first: Cell::new(None),
        stale: Cell::new(true),
        watch: Watch::new(),
    });
    let held = Rc::clone(&rows);
    create_binding(move || held.set_direction(direction.get()));
    let held = Rc::clone(&rows);
    create_binding(move || held.set_estimate(item_size.get()));
    let held = Rc::clone(&rows);
    create_binding(move || held.overscan.set(overscan.get().max(0.0)));
    let held = Rc::clone(&rows);
    create_binding(move || {
        let keys = keys.get();
        untrack(|| held.set_keys(keys));
    });
    untrack(|| rows.lay_out());
    let held = Rc::clone(&rows);
    on_cleanup(move || drop(held.items.retain(&[])));
    beui_core_html::frame::on_layout(Rc::new(move || rows.lay_out()));
    list
}
