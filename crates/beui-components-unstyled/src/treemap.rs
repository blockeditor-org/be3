use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;

use accesskit::{Node, Role};
use be_lib_stable_hash::HashMap;
use beui_macros::{component, view};

use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::{CanvasArrangement, CursorIcon, Key, KeyPress, Pos2, Rect, Vec2};

/// The most tiles one level shows; the smallest of the rest share one more tile,
/// so a level of any length lays out in time linear in its items.
pub const TREEMAP_MOST_TILES: usize = 40;

/// One thing a level of a treemap shows, sized by `size`. `opens` says it has a
/// level of its own to go into; its size is its own data, never a sum of what is
/// below it, so a treemap needs only the levels it shows.
#[derive(Clone, PartialEq, Debug)]
pub struct TreemapItem<K> {
    pub key: K,
    pub label: String,
    pub size: u64,
    pub opens: bool,
}

/// A tile: one item, or the rest of a level past `TREEMAP_MOST_TILES`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum TreemapTileKey<K> {
    Item(K),
    Rest,
}

pub type TreemapTile<K> = TreemapItem<TreemapTileKey<K>>;

/// What a treemap's face arranges around its tiles.
pub struct TreemapHandle<K: 'static> {
    /// The tiles, filling whatever room the face gives them.
    pub tiles: NodeId,
    /// The levels gone into, outermost first.
    pub path: Memo<Vec<TreemapItem<K>>>,
    /// Back out one level; nothing at the top.
    pub up: Callback<()>,
    /// Back out to the first `n` levels of `path` (0 is the top).
    pub climb: Callback<usize>,
    /// The tile under the pointer, else the one with focus.
    pub shown: Memo<Option<TreemapTile<K>>>,
    /// The sum of the level's sizes.
    pub total: Memo<u64>,
}

/// What a tile's face paints from.
pub struct TreemapTileHandle<K: 'static> {
    pub tile: Memo<TreemapTile<K>>,
    /// Its laid-out size, once laid out: for showing a label only where it fits.
    pub size: Memo<Option<Vec2>>,
    pub focused: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
}

type Placed<K> = Rc<RefCell<Vec<(TreemapTileKey<K>, Rect)>>>;

struct State<K: 'static> {
    path: Binding<Vec<TreemapItem<K>>>,
    tiles: Memo<Vec<TreemapTile<K>>>,
    focus: Signal<Option<TreemapTileKey<K>>>,
    hovered: Signal<Option<TreemapTileKey<K>>>,
    refocus: Signal<bool>,
    placed: Stored<Placed<K>>,
    ids: Stored<RefCell<Ids<K>>>,
}

impl<K> Clone for State<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for State<K> {}

struct Ids<K> {
    next: u64,
    of: HashMap<TreemapTileKey<K>, u64>,
}

/// A level of sized things as nested rectangles, each with an area by its size,
/// laid out squarified (Bruls, Huizing and van Wijk) so tiles stay near square.
/// Going into a tile that `opens` shows its level in the same room; the caller
/// derives `items` from `path`, so it lists a level only once it is gone into.
///
/// One Tab stop, at the focused tile (else the largest). The arrow keys move to
/// the nearest tile that way, Home and End to the largest and smallest; Enter
/// or Space, or a press, goes into a tile that opens; Backspace or Alt+Up goes
/// back out. A press on a tile that does not open only focuses it, so `shown`
/// names it.
#[component]
pub fn Treemap<K>(
    items: Prop<Vec<TreemapItem<K>>>,
    path: Binding<Vec<TreemapItem<K>>>,
    /// Says a size in words, for each tile's accessible name.
    describe: Func<u64, String>,
    /// The room between neighbouring tiles.
    gap: Prop<f32>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::from("treemap.tile"))] tile_test_id: String,
    tile: RenderFn<TreemapTileHandle<K>>,
    #[prop(children)] content: Render<TreemapHandle<K>>,
) -> NodeId
where
    K: Clone + Eq + Hash + std::fmt::Display + 'static,
{
    let tiles = create_memo(move || items.with(|items| shown_tiles(items, TREEMAP_MOST_TILES)));
    let keys = create_memo(move || {
        tiles.with(|tiles| {
            tiles
                .iter()
                .map(|tile| tile.key.clone())
                .collect::<Vec<_>>()
        })
    });
    let total = create_memo(move || tiles.with(|tiles| tiles.iter().map(|tile| tile.size).sum()));
    let state = State {
        path,
        tiles,
        focus: create_signal(None),
        hovered: create_signal(None),
        refocus: create_signal(false),
        placed: store(Rc::default()),
        ids: store(RefCell::new(Ids {
            next: 0,
            of: HashMap::default(),
        })),
    };
    create_effect(move || {
        if !state.refocus.get() {
            return;
        }
        if let Some(first) = keys.with(|keys| keys.first().cloned()) {
            state.refocus.set(false);
            state.focus.set(Some(first));
        }
    });
    let focused = create_selector(move || state.focus.get());
    let tab_stop = create_memo(move || {
        let present = state
            .focus
            .get()
            .filter(|key| keys.with(|keys| keys.contains(key)));
        present.or_else(|| keys.with(|keys| keys.first().cloned()))
    });
    let tab_stops = create_selector(move || tab_stop.get());
    let shown = create_memo(move || {
        let key = state.hovered.get().or_else(|| state.focus.get())?;
        tiles.with(|tiles| tiles.iter().find(|tile| tile.key == key).cloned())
    });
    let gap = create_memo(move || gap.get());
    let arrangement = create_memo(move || {
        let sizes = tiles.with(|tiles| {
            tiles
                .iter()
                .map(|tile| (tile.key.clone(), tile.size as f64))
                .collect::<Vec<_>>()
        });
        let gap = gap.get();
        let placed = state.placed.value();
        let ids = state.ids;
        Some(CanvasArrangement::new(move |size| {
            let rects = squarify(
                &sizes.iter().map(|(_, size)| *size).collect::<Vec<_>>(),
                Rect::from_min_size(Pos2::ZERO, size),
            );
            let mut placed = placed.borrow_mut();
            placed.clear();
            let ids = ids.value();
            let ids = ids.borrow();
            sizes
                .iter()
                .zip(rects)
                .filter_map(|((key, _), rect)| {
                    let rect = rect.shrink(gap / 2.0);
                    placed.push((key.clone(), rect));
                    Some((*ids.of.get(key)?, rect))
                })
                .collect()
        }))
    });
    let path_memo = create_memo(move || path.get());
    let up = Callback::new(move |()| go_up(state));
    let climb = Callback::new(move |levels: usize| {
        let refocus = state.focus.get_untracked().is_some();
        state.path.update(|path| path.truncate(levels));
        state.refocus.set(refocus);
    });
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::List);
        let label = label.get();
        if !label.is_empty() {
            node.set_label(label);
        }
        node
    }));
    let canvas = view! {
        <Canvas arrangement>
            <ForEach keys>|key: TreemapTileKey<K>| {
                let tile_id = match &key {
                    TreemapTileKey::Item(key) => format!("{tile_test_id}.{key}"),
                    TreemapTileKey::Rest => format!("{tile_test_id}.rest"),
                };
                let id = arranged_id(state, &key);
                view! {
                    <CanvasItem arranged={id}>
                        <TreemapTileView
                            @test_id={tile_id}
                            tile_key={key}
                            state
                            describe
                            tab_stops
                            focused
                            tile
                        />
                    </CanvasItem>
                }
            }</ForEach>
        </Canvas>
    };
    content.call(TreemapHandle {
        tiles: canvas,
        path: path_memo,
        up,
        climb,
        shown,
        total,
    })
}

#[component]
fn TreemapTileView<K>(
    tile_key: TreemapTileKey<K>,
    state: State<K>,
    describe: Func<u64, String>,
    tab_stops: Selector<Option<TreemapTileKey<K>>>,
    focused: Selector<Option<TreemapTileKey<K>>>,
    tile: RenderFn<TreemapTileHandle<K>>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
{
    let data = create_memo({
        let key = tile_key.clone();
        move || {
            state.tiles.with(|tiles| {
                tiles
                    .iter()
                    .find(|tile| tile.key == key)
                    .cloned()
                    .unwrap_or_else(|| TreemapItem {
                        key: key.clone(),
                        label: String::new(),
                        size: 0,
                        opens: false,
                    })
            })
        }
    });
    component_accessibility(create_memo(move || {
        let tile = data.get();
        let mut node = Node::new(Role::ListItem);
        node.set_label(format!("{}, {}", tile.label, describe.call(tile.size)));
        if tile.opens {
            node.set_expanded(false);
        }
        node
    }));
    let has_focus = create_signal(false);
    let hovered = create_signal(false);
    let active = create_signal(false);
    let built = NodeRef::new();
    let laid_out = create_signal(None::<Vec2>);
    let size = create_memo(move || laid_out.get());
    let open = Callback::new(move |()| {
        let tile = data.get_untracked();
        if let (true, TreemapTileKey::Item(key)) = (tile.opens, tile.key) {
            go_in(
                state,
                TreemapItem {
                    key,
                    label: tile.label,
                    size: tile.size,
                    opens: true,
                },
            );
        }
    });
    let (focus_key, press_key, hover_key) = (tile_key.clone(), tile_key.clone(), tile_key.clone());
    let tile_key_for_focus = tile_key.clone();
    let face = tile.call(TreemapTileHandle {
        tile: data,
        size,
        focused: has_focus.read_only(),
        hovered: hovered.read_only(),
        active: active.read_only(),
    });
    let item = view! {
        <Interactive
            @node_ref=&built
            focusable=true
            cursor=CursorIcon::PointingHand
            tab_stop={tab_stops.memo(Some(tile_key.clone()))}
            focused={focused.memo(Some(tile_key))}
            on_focus_change={move |now: bool| {
                has_focus.set(now);
                if now {
                    state.focus.set(Some(focus_key.clone()));
                } else if state.focus.get_untracked().as_ref() == Some(&focus_key) {
                    state.focus.set(None);
                }
            }}
            on_hover_change={move |over: bool| {
                hovered.set(over);
                if over {
                    state.hovered.set(Some(hover_key.clone()));
                } else if state.hovered.get_untracked().as_ref() == Some(&hover_key) {
                    state.hovered.set(None);
                }
            }}
            on_active_change={move |down: bool| active.set(down)}
            on_activate={open}
            on_click={open}
            on_key={move |press: KeyPress| key_press(state, &press_key, press)}
        >
            {face}
        </Interactive>
    };
    let measured = measure(built.get());
    let placed_key = tile_key_for_focus;
    create_effect(move || {
        let Some(at) = measured.get() else {
            return;
        };
        let first = laid_out.get_untracked().is_none();
        laid_out.set(Some(at.size));
        let wanted = state.focus.get_untracked().as_ref() == Some(&placed_key);
        if first && wanted && !has_focus.get_untracked() {
            built.get().focus();
        }
    });
    item
}

/// The key a tile's `CanvasItem` is arranged by, fresh for each tile built and
/// forgotten with it.
fn arranged_id<K: Clone + Eq + Hash + 'static>(state: State<K>, key: &TreemapTileKey<K>) -> u64 {
    let id = state.ids.with(|ids| {
        let mut ids = ids.borrow_mut();
        let id = ids.next;
        ids.next += 1;
        ids.of.insert(key.clone(), id);
        id
    });
    let forget = key.clone();
    on_cleanup(move || {
        state.ids.try_with(|ids| {
            let mut ids = ids.borrow_mut();
            if ids.of.get(&forget) == Some(&id) {
                ids.of.remove(&forget);
            }
        });
    });
    id
}

fn go_in<K: Clone + Eq + Hash + 'static>(state: State<K>, item: TreemapItem<K>) {
    let refocus = state.focus.get_untracked().is_some();
    state.hovered.set(None);
    state.path.update(|path| path.push(item));
    state.refocus.set(refocus);
}

fn go_up<K: Clone + Eq + Hash + 'static>(state: State<K>) {
    let had_focus = state.focus.get_untracked().is_some();
    let left = state.path.update(Vec::pop);
    state.hovered.set(None);
    if let (true, Some(left)) = (had_focus, left) {
        state.focus.set(Some(TreemapTileKey::Item(left.key)));
    }
}

fn key_press<K>(state: State<K>, key: &TreemapTileKey<K>, press: KeyPress) -> bool
where
    K: Clone + Eq + Hash + 'static,
{
    let back = press.key == Key::Backspace || (press.modifiers.alt && press.key == Key::ArrowUp);
    if back {
        if press.pressed {
            go_up(state);
        }
        return true;
    }
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let toward = match press.key {
        Key::ArrowLeft => Vec2::new(-1.0, 0.0),
        Key::ArrowRight => Vec2::new(1.0, 0.0),
        Key::ArrowUp => Vec2::new(0.0, -1.0),
        Key::ArrowDown => Vec2::new(0.0, 1.0),
        Key::Home | Key::End => {
            if press.pressed {
                let end = state.tiles.with_untracked(|tiles| match press.key {
                    Key::Home => tiles.first().map(|tile| tile.key.clone()),
                    _ => tiles.last().map(|tile| tile.key.clone()),
                });
                if end.is_some() {
                    state.focus.set(end);
                }
            }
            return true;
        }
        _ => return false,
    };
    if press.pressed {
        let next = state.placed.with(|placed| {
            let placed = placed.borrow();
            let from = placed.iter().find(|(tile, _)| tile == key)?.1;
            nearest(from, toward, placed.iter().filter(|(tile, _)| tile != key))
        });
        if let Some(next) = next {
            state.focus.set(Some(next));
        }
    }
    true
}

/// The tile past `from` toward `toward` whose near edge is closest, counting
/// how far its centre strays across that axis.
fn nearest<'a, K: Clone + 'a>(
    from: Rect,
    toward: Vec2,
    candidates: impl Iterator<Item = &'a (TreemapTileKey<K>, Rect)>,
) -> Option<TreemapTileKey<K>> {
    let centre = from.center();
    candidates
        .filter_map(|(key, rect)| {
            let at = rect.center();
            let (ahead, gap, across) = match (toward.x, toward.y) {
                (x, _) if x > 0.0 => (at.x - centre.x, rect.left() - from.right(), at.y - centre.y),
                (x, _) if x < 0.0 => (centre.x - at.x, from.left() - rect.right(), at.y - centre.y),
                (_, y) if y > 0.0 => (at.y - centre.y, rect.top() - from.bottom(), at.x - centre.x),
                _ => (centre.y - at.y, from.top() - rect.bottom(), at.x - centre.x),
            };
            (ahead > 0.0).then(|| (key.clone(), gap.max(0.0) + across.abs()))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(key, _)| key)
}

/// The tiles a level shows, largest first: every item with a size, up to
/// `most`, and one tile for the rest. Linear in `items`: the largest are
/// selected, not the whole level sorted.
pub fn shown_tiles<K: Clone>(items: &[TreemapItem<K>], most: usize) -> Vec<TreemapTile<K>> {
    let mut sized: Vec<&TreemapItem<K>> = items.iter().filter(|item| item.size > 0).collect();
    let rest = match sized.len() > most {
        true => {
            let keep = most.saturating_sub(1);
            sized.select_nth_unstable_by(keep, |a, b| b.size.cmp(&a.size));
            let rest: Vec<&TreemapItem<K>> = sized.split_off(keep);
            Some(TreemapItem {
                key: TreemapTileKey::Rest,
                label: format!("{} more", rest.len()),
                size: rest.iter().map(|item| item.size).sum(),
                opens: false,
            })
        }
        false => None,
    };
    sized.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.label.cmp(&b.label)));
    sized
        .into_iter()
        .map(|item| TreemapItem {
            key: TreemapTileKey::Item(item.key.clone()),
            label: item.label.clone(),
            size: item.size,
            opens: item.opens,
        })
        .chain(rest)
        .collect()
}

/// Squarified rectangles filling `area`, one per size, in order; `sizes` must
/// be largest first. Each size is visited a constant number of times.
pub fn squarify(sizes: &[f64], area: Rect) -> Vec<Rect> {
    let total: f64 = sizes.iter().sum();
    if total <= 0.0 || area.width() <= 0.0 || area.height() <= 0.0 {
        return vec![Rect::from_min_size(area.min, Vec2::ZERO); sizes.len()];
    }
    let scale = f64::from(area.width()) * f64::from(area.height()) / total;
    let (mut x, mut y) = (f64::from(area.min.x), f64::from(area.min.y));
    let (mut width, mut height) = (f64::from(area.width()), f64::from(area.height()));
    let mut rects = Vec::with_capacity(sizes.len());
    let mut start = 0;
    while start < sizes.len() {
        let short = width.min(height);
        let largest = sizes[start] * scale;
        let mut sum = 0.0;
        let mut worst = f64::INFINITY;
        let mut end = start;
        while end < sizes.len() {
            let next = sum + sizes[end] * scale;
            let ratio = worst_ratio(largest, sizes[end] * scale, next, short);
            if end > start && ratio > worst {
                break;
            }
            sum = next;
            worst = ratio;
            end += 1;
        }
        let last = end == sizes.len();
        let across = match short > 0.0 {
            true => sum / short,
            false => 0.0,
        };
        let mut along = 0.0;
        for (index, size) in sizes[start..end].iter().enumerate() {
            let length = match (last && index == end - start - 1, across > 0.0) {
                (true, _) => short - along,
                (false, true) => size * scale / across,
                (false, false) => 0.0,
            };
            let rect = match width >= height {
                true => Rect::from_min_size(
                    Pos2::new(x as f32, (y + along) as f32),
                    Vec2::new(across as f32, length as f32),
                ),
                false => Rect::from_min_size(
                    Pos2::new((x + along) as f32, y as f32),
                    Vec2::new(length as f32, across as f32),
                ),
            };
            rects.push(rect);
            along += length;
        }
        match width >= height {
            true => {
                x += across;
                width = (width - across).max(0.0);
            }
            false => {
                y += across;
                height = (height - across).max(0.0);
            }
        }
        start = end;
    }
    rects
}

fn worst_ratio(largest: f64, smallest: f64, sum: f64, short: f64) -> f64 {
    let (sum2, short2) = (sum * sum, short * short);
    if sum2 <= 0.0 || smallest <= 0.0 {
        return f64::INFINITY;
    }
    (short2 * largest / sum2).max(sum2 / (short2 * smallest))
}

#[cfg(test)]
mod tests;
