use be_lib_stable_hash::HashMap;
use std::cell::{Cell, RefCell};
use std::hash::Hash;
use std::rc::Rc;

use crate::reactive::Callback;
use crate::reactive::ForEachFill;
use crate::reactive::ForEachRef;
use crate::reactive::Get;
use crate::reactive::KeyedItems;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::ReadSignal;
use crate::reactive::RenderFn;
use crate::reactive::ScopeContext;
use crate::reactive::Set;
use crate::reactive::ShownRow;
use crate::reactive::Signal;
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
use beui_core_html::rows::{Watch, follow_row, main_length, spacer, start_within, visible_span};
use beui_reactive::numbered;

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
    /// The row its owner keeps in place (the `anchor` prop), by its numbered key.
    anchor: RefCell<Option<(K, usize)>>,
    /// The row kept in place, its node and where it started along the list when last seen:
    /// the one named `anchor` while built, else the first in view.
    kept: RefCell<Option<((K, usize), NodeId, f32)>>,
    watch: Watch,
    /// The place each built row was handed, by its numbered key.
    places: Rc<RefCell<HashMap<(K, usize), Signal<usize>>>>,
    /// The handle this list fills, if given one: the keys as they change, the rows each
    /// time the rows built change, at a layout pass, as natively.
    fill: Option<ForEachFill<K, NodeId>>,
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
        if let Some(fill) = self.fill {
            fill.keys(keys.iter().map(|(key, _)| key.clone()).collect());
        }
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
        self.place(range);
    }

    /// Hands each built row its place, forgets the rows no longer built, and tells the
    /// handle, if any.
    fn place(&self, (start, end): (usize, usize)) {
        let keys = self.keys.borrow();
        let built = &keys[start.min(keys.len())..end.min(keys.len())];
        let mut places = self.places.borrow_mut();
        let kept: be_lib_stable_hash::HashSet<&(K, usize)> = built.iter().collect();
        places.retain(|key, _| kept.contains(key));
        for (at, key) in built.iter().enumerate() {
            if let Some(place) = places.get(key)
                && place.get_untracked() != start + at
            {
                place.set(start + at);
            }
        }
        let Some(fill) = self.fill else {
            return;
        };
        let nodes = self.nodes.borrow();
        let rows = built
            .iter()
            .zip(nodes.iter())
            .filter_map(|(key, node)| {
                let place = places.get(key)?;
                Some(ShownRow {
                    key: key.0.clone(),
                    index: place.read_only(),
                    nodes: vec![*node],
                })
            })
            .collect();
        drop((places, nodes, keys));
        fill.rows(rows);
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

    /// Scrolls on with the kept row wherever rows measured anew above it moved it, where the
    /// browser does not (`follow_row`).
    fn hold(&self) {
        let Some((key, node, start)) = self.kept.borrow_mut().take() else {
            return;
        };
        let (from, _) = self.shown.get();
        let built = self.nodes.borrow().iter().position(|built| *built == node);
        let same = built.is_some_and(|at| self.keys.borrow().get(from + at) == Some(&key));
        let direction = self.direction.get();
        let Some(now) = start_within(self.list, node, direction).filter(|_| same) else {
            return;
        };
        if (now - start).abs() >= 0.5 {
            follow_row(self.list, direction, now - start);
        }
        *self.kept.borrow_mut() = Some((key, node, now));
    }

    fn keep(&self, window: Option<(f32, f32)>) {
        let (from, to) = self.shown.get();
        let keys = self.keys.borrow();
        let named = self.anchor.borrow().as_ref().and_then(|anchor| {
            let at = keys.get(from..to)?.iter().position(|key| key == anchor)?;
            Some(from + at)
        });
        let at = named.or_else(|| {
            let (first, _) = window?;
            let index = self.metrics.borrow().index_at(first);
            (from..to).contains(&index).then_some(index)
        });
        let kept = at.and_then(|at| {
            let node = *self.nodes.borrow().get(at - from)?;
            let start = start_within(self.list, node, self.direction.get())?;
            Some((keys.get(at)?.clone(), node, start))
        });
        *self.kept.borrow_mut() = kept;
    }

    fn lay_out(&self) -> bool {
        let mut changed = self.measure();
        self.hold();
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
            self.hold();
        }
        self.keep(visible_span(self.list, self.direction.get()));
        changed
    }
}

pub(crate) fn virtual_list<K>(
    keys: Prop<Vec<K>>,
    item_size: Prop<f32>,
    direction: Prop<Direction>,
    overscan: Prop<f32>,
    anchor: Prop<Option<K>>,
    text: Callback<K, String>,
    handle: Option<ForEachRef<K, NodeId>>,
    item: RenderFn<(ReadSignal<K>, ReadSignal<usize>), NodeId>,
) -> NodeId
where
    K: Clone + Hash + Eq + 'static,
{
    let places = Rc::new(RefCell::new(HashMap::<(K, usize), Signal<usize>>::default()));
    let placing = Rc::clone(&places);
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
        items: KeyedItems::new(move |numbered: (K, usize)| {
            let place = create_signal(0);
            placing.borrow_mut().insert(numbered.clone(), place);
            item.call((create_signal(numbered.0).read_only(), place.read_only()))
        }),
        shown: Cell::new((0, 0)),
        nodes: RefCell::default(),
        overscan: Cell::new(0.0),
        first: Cell::new(None),
        stale: Cell::new(true),
        anchor: RefCell::default(),
        kept: RefCell::default(),
        watch: Watch::new(),
        places,
        fill: handle.map(|handle| handle.claim()),
    });
    let held = Rc::clone(&rows);
    create_binding(move || held.set_direction(direction.get()));
    let held = Rc::clone(&rows);
    create_binding(move || held.set_estimate(item_size.get()));
    let held = Rc::clone(&rows);
    create_binding(move || held.overscan.set(overscan.get().max(0.0)));
    let held = Rc::clone(&rows);
    create_binding(move || *held.anchor.borrow_mut() = anchor.get().map(|key| (key, 0)));
    let held = Rc::clone(&rows);
    create_binding(move || {
        let keys = keys.get();
        untrack(|| held.set_keys(keys));
    });
    untrack(|| rows.lay_out());
    beui_core_html::document::set_state(list, Box::new(Rc::clone(&rows)));
    let held = Rc::clone(&rows);
    on_cleanup(move || drop(held.items.retain(&[])));
    beui_core_html::frame::on_layout(Rc::new(move || rows.lay_out()));
    list
}

/// Where the row of `key` starts along `list`, from the sizes measured so far, as natively.
pub(crate) fn row_offset<K: Clone + Hash + Eq + 'static>(list: NodeId, key: &K) -> Option<f32> {
    let rows = beui_core_html::document::state::<Rc<Rows<K>>>(list)?;
    let index = rows
        .keys
        .borrow()
        .iter()
        .position(|numbered| numbered.0 == *key && numbered.1 == 0)?;
    Some(rows.metrics.borrow().position(index))
}
