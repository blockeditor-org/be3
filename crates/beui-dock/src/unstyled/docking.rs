use beui_view::types::icons::Glyph;
mod host;
mod switch;

use stable_hash::{HashMap, HashSet};
use std::any::Any;
use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;

use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Color;
use beui_view::types::Direction;
use beui_view::types::Image;
use beui_view::types::Modifiers;
use beui_view::types::{Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};

use super::dialog::{DialogParts, DockDialogHandle};
use super::state::{
    DockAlign, DockBarPlacement, DockEdge, DockFullscreen, DockSpec, DockSpecBar, DockSpecEntry,
    DockSpecNode, DockSpecPane, DockSpecWindow, joined_host, joined_key, joined_prefix,
};
use super::{
    Dock, DockApi, DockAwayHandle, DockBarBandHandle, DockBarControl, DockBarHandle,
    DockChromeHandle, DockConfig, DockGripHandle, DockMode, DockPreviewHandle,
    DockSplitterHandle, DockStackHandle, DockState, DockStripHandle, DockSwitchHandle,
    DockSwitcherHandle, DockTabControl, DockTabHandle, DockTabMenu, GroupId, LeafId, SIDEBAR_WIDTH,
    TabId,
};
use beui_components_unstyled::DialogLayout;
use beui_components_unstyled::menu_popup::MenuStyle;
const MAX_JOINED_DEPTH: usize = 16;

use super::bars::shared_key;
use super::{BarCovers, BarFloats};
pub use host::{
    DockBarMove, DockBodies, DockDeclaration, DockDeclaredBar, DockDeclaredDialog, DockDeclaredTab,
    DockDragStage, DockHostEvent, DockHostLink, DockHostSlot, DockHosted, DockJoin, DockKeeper,
    DockKept, DockRequest, use_dock_join,
};
use host::{Joined, Registry};

pub trait DockKey: Clone + Eq + Hash + 'static {
    fn key(&self) -> String;
}

impl DockKey for TabId {
    fn key(&self) -> String {
        self.value().to_string()
    }
}

impl DockKey for u64 {
    fn key(&self) -> String {
        self.to_string()
    }
}

impl DockKey for String {
    fn key(&self) -> String {
        self.clone()
    }
}

impl DockKey for &'static str {
    fn key(&self) -> String {
        (*self).to_owned()
    }
}

pub struct DockNode<K: 'static> {
    kind: NodeKind<K>,
}

enum NodeKind<K: 'static> {
    Split {
        key: String,
        direction: Direction,
        fraction: Memo<f32>,
        children: Run<DockNode<K>>,
    },
    Pane {
        key: String,
        active: Option<K>,
        vertical: bool,
        icons_only: bool,
        sidebar: f32,
        locked: bool,
        bare: bool,
        empty: Option<RenderFn<()>>,
        entries: Run<DockEntry<K>>,
    },
    Window {
        key: String,
        rect: Rect,
        children: Run<DockNode<K>>,
    },
    Dialog {
        id: K,
        parent: K,
        parts: DialogParts,
    },
    Bar {
        id: K,
        parent: Option<K>,
        placement: DockBarPlacement,
        content: RenderFn<()>,
    },
}

pub struct DockEntry<K: 'static> {
    kind: EntryKind<K>,
}

enum EntryKind<K: 'static> {
    Tab {
        id: K,
        title: Memo<String>,
        icon: Memo<Option<Glyph>>,
        picture: Memo<Option<Image>>,
        on_close: Option<ClickCallback>,
        see_through: bool,
        content: RenderFn<()>,
    },
    Group {
        key: String,
        title: Option<Memo<String>>,
        pinned: bool,
        children: Run<DockNode<K>>,
    },
}

beui_view::value_child_type!(<K> DockNode<K>);
beui_view::value_child_type!(<K> DockEntry<K>);

#[component]
pub fn DockSplit<K>(
    id: String,
    #[prop(default = Direction::Horizontal)] direction: Direction,
    #[prop(default = 0.5)] fraction: Prop<f32>,
    children: Children<DockNode<K>>,
) -> DockNode<K>
where
    K: DockKey,
{
    DockNode {
        kind: NodeKind::Split {
            key: id,
            direction,
            fraction: create_memo(move || fraction.get()),
            children: children.into_run(),
        },
    }
}

#[component]
pub fn DockPane<K>(
    id: String,
    active: Option<K>,
    #[prop(default = false)] vertical: bool,
    #[prop(default = false)] icons_only: bool,
    #[prop(default = SIDEBAR_WIDTH)] sidebar_width: f32,
    #[prop(default = false)] locked: bool,
    #[prop(default = false)] bare: bool,
    empty: Option<RenderFn<()>>,
    children: Children<DockEntry<K>>,
) -> DockNode<K>
where
    K: DockKey,
{
    DockNode {
        kind: NodeKind::Pane {
            key: id,
            active,
            vertical,
            icons_only,
            sidebar: sidebar_width,
            locked,
            bare,
            empty,
            entries: children.into_run(),
        },
    }
}

#[component]
pub fn DockWindow<K>(id: String, rect: Rect, children: Children<DockNode<K>>) -> DockNode<K>
where
    K: DockKey,
{
    DockNode {
        kind: NodeKind::Window {
            key: id,
            rect,
            children: children.into_run(),
        },
    }
}

#[component]
pub fn DockDialog<K>(
    id: K,
    parent: K,
    title: Prop<String>,
    #[prop(default = None)] icon: Prop<Option<Glyph>>,
    size: Prop<Vec2>,
    #[prop(default = false)] modal: Prop<bool>,
    #[prop(default = false)] dismiss_on_outside_press: Prop<bool>,
    on_close: Option<ClickCallback>,
    #[prop(children)] content: RenderFn<()>,
) -> DockNode<K>
where
    K: DockKey,
{
    DockNode {
        kind: NodeKind::Dialog {
            id,
            parent,
            parts: DialogParts {
                title: create_memo(move || title.get()),
                icon: create_memo(move || icon.get()),
                size: create_memo(move || size.get()),
                modal: create_memo(move || modal.get()),
                dismiss_on_outside_press: create_memo(move || dismiss_on_outside_press.get()),
                on_close,
                content,
            },
        },
    }
}

#[component]
pub fn DockBar<K>(
    id: K,
    parent: Option<K>,
    #[prop(default = DockBarPlacement::Over(DockEdge::Top, DockAlign::Center))]
    placement: DockBarPlacement,
    #[prop(children)] content: RenderFn<()>,
) -> DockNode<K>
where
    K: DockKey,
{
    DockNode {
        kind: NodeKind::Bar {
            id,
            parent,
            placement,
            content,
        },
    }
}

#[component]
pub fn DockTab<K>(
    id: K,
    title: Prop<String>,
    #[prop(default = None)] icon: Prop<Option<Glyph>>,
    #[prop(default = None)] picture: Prop<Option<Image>>,
    on_close: Option<ClickCallback>,
    #[prop(default = false)] see_through: bool,
    #[prop(children)] content: RenderFn<()>,
) -> DockEntry<K>
where
    K: DockKey,
{
    DockEntry {
        kind: EntryKind::Tab {
            id,
            title: create_memo(move || title.get()),
            icon: create_memo(move || icon.get()),
            picture: create_memo(move || picture.get()),
            on_close,
            see_through,
            content,
        },
    }
}

#[component]
pub fn DockGroup<K>(
    id: String,
    title: Option<Prop<String>>,
    #[prop(default = false)] pinned: bool,
    children: Children<DockNode<K>>,
) -> DockEntry<K>
where
    K: DockKey,
{
    DockEntry {
        kind: EntryKind::Group {
            key: id,
            title: title.map(|title| create_memo(move || title.get())),
            pinned,
            children: children.into_run(),
        },
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DockingSnapshot<K> {
    state: DockState,
    keys: Vec<(K, TabId)>,
}

impl<K> DockingSnapshot<K> {
    pub fn new(state: DockState, keys: impl IntoIterator<Item = (K, TabId)>) -> Self {
        Self {
            state,
            keys: keys.into_iter().collect(),
        }
    }

    pub fn state(&self) -> &DockState {
        &self.state
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.keys.iter().map(|(key, _)| key)
    }
}

struct KeyMap<K> {
    tabs: HashMap<K, TabId>,
    keys: HashMap<TabId, K>,
    next: u64,
    changed: bool,
}

impl<K> Default for KeyMap<K> {
    fn default() -> Self {
        Self {
            tabs: HashMap::default(),
            keys: HashMap::default(),
            next: 1,
            changed: true,
        }
    }
}

struct ForeignDialog {
    described: Signal<DockDeclaredDialog>,
    parts: DialogParts,
    _scope: Scope,
}

struct LayoutInner<K> {
    _scope: Scope,
    keys: RefCell<KeyMap<K>>,
    keyed: Signal<u64>,
    registry: Rc<Registry>,
    host: RefCell<Option<Rc<dyn DockHostLink>>>,
    hosted: Signal<Option<DockHosted>>,
    declared: RefCell<Option<DockSpec>>,
    dialogs: RefCell<HashMap<TabId, ForeignDialog>>,
    place: std::cell::Cell<Option<RenderFn<TabId, NodeId>>>,
    bars_link: RefCell<Option<Rc<dyn DockHostLink>>>,
    bars_hosted: Signal<Option<DockHosted>>,
}

pub struct DockingLayout<K: 'static> {
    state: Signal<DockState>,
    mode: Signal<DockMode>,
    restores: Signal<u64>,
    covers: Signal<BarCovers>,
    floats: Signal<BarFloats>,
    covered: Signal<bool>,
    area: Signal<Rect>,
    inner: Stored<LayoutInner<K>>,
}

impl<K> Clone for DockingLayout<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for DockingLayout<K> {}

impl<K: DockKey> Default for DockingLayout<K> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct OwnedDockingLayout<K: 'static> {
    layout: DockingLayout<K>,
    _inner: OwnedStored<LayoutInner<K>>,
}

impl<K: DockKey> Default for OwnedDockingLayout<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: DockKey> OwnedDockingLayout<K> {
    pub fn new() -> Self {
        let (layout, inner) = DockingLayout::parts();
        let inner = OwnedStored::new(inner);
        Self {
            layout: layout(inner.handle()),
            _inner: inner,
        }
    }

    pub fn layout(&self) -> DockingLayout<K> {
        self.layout
    }
}

impl<K: 'static> std::ops::Deref for OwnedDockingLayout<K> {
    type Target = DockingLayout<K>;

    fn deref(&self) -> &DockingLayout<K> {
        &self.layout
    }
}

impl<K: DockKey> DockingLayout<K> {
    pub fn new() -> Self {
        let (layout, inner) = Self::parts();
        layout(store(inner))
    }

    fn parts() -> (impl FnOnce(Stored<LayoutInner<K>>) -> Self, LayoutInner<K>) {
        let owner = RootScope::new();
        let state = create_signal(DockState::default());
        let mode = create_signal(DockMode::Tiled);
        let restores = create_signal(0);
        let hosted = create_signal(None);
        let bars_hosted = create_signal(None);
        let covers = create_signal(BarCovers::default());
        let floats = create_signal(BarFloats::default());
        let covered = create_signal(false);
        let area = create_signal(Rect::ZERO);
        let registry = Rc::new(Registry {
            covers,
            version: create_signal(0),
            joins: RefCell::default(),
            foreign: RefCell::default(),
            state,
            mode,
            above: hosted,
            host: RefCell::default(),
            dock: std::cell::Cell::new(None),
        });
        let inner = LayoutInner {
            _scope: owner.into_scope(),
            keys: RefCell::default(),
            keyed: create_signal(0),
            registry,
            host: RefCell::default(),
            hosted,
            declared: RefCell::default(),
            dialogs: RefCell::default(),
            place: std::cell::Cell::new(None),
            bars_link: RefCell::default(),
            bars_hosted,
        };
        let layout = move |inner| Self {
            state,
            mode,
            restores,
            covers,
            floats,
            covered,
            area,
            inner,
        };
        (layout, inner)
    }

    fn inner(&self) -> Rc<LayoutInner<K>> {
        self.inner.value()
    }

    fn host(&self) -> Option<Rc<dyn DockHostLink>> {
        self.inner().host.borrow().clone()
    }

    fn hosted<R>(&self, read: impl FnOnce(&DockHosted) -> R) -> Option<R> {
        self.inner().hosted.with(|hosted| hosted.as_ref().map(read))
    }

    fn fresh_keys(&self) -> KeyMap<K> {
        let mut map = KeyMap::default();
        map.next = self.inner().registry.foreign.borrow().newest() + 1;
        map
    }

    pub fn restore(&self, snapshot: DockingSnapshot<K>) {
        let DockingSnapshot { mut state, keys } = snapshot;
        state.mark_seeded();
        let inner = self.inner();
        let mut map = self.fresh_keys();
        for (key, tab) in keys {
            map.next = map.next.max(tab.value() + 1);
            map.keys.insert(tab, key.clone());
            map.tabs.insert(key, tab);
        }
        *inner.keys.borrow_mut() = map;
        match self.host() {
            Some(host) => host.request(DockRequest::Restore(state)),
            None => {
                inner.registry.keep_joined();
                self.state.set(state);
            }
        }
        self.restores.update(|restores| *restores += 1);
    }

    fn keep_state(&self, mut state: DockState) {
        state.mark_seeded();
        self.inner().registry.keep_joined();
        self.state.set(state);
        self.restores.update(|restores| *restores += 1);
    }

    pub fn reset(&self) {
        match self.host() {
            Some(host) => {
                let declared = self.inner().declared.borrow().clone();
                if let Some(spec) = declared {
                    let mut fresh = DockState::default();
                    fresh.reconcile(&spec);
                    host.request(DockRequest::Restore(fresh));
                }
            }
            None => {
                let keys = self.fresh_keys();
                *self.inner().keys.borrow_mut() = keys;
                self.inner().registry.keep_joined();
                self.state.set(DockState::default());
            }
        }
        self.restores.update(|restores| *restores += 1);
    }

    pub fn snapshot(&self) -> DockingSnapshot<K> {
        let mut state = self.state.get();
        let inner = self.inner();
        while let Some((group, host)) = state.joined_groups().first().copied() {
            state.unhost_group(group, host);
        }
        let foreign: Vec<TabId> = {
            let registry = inner.registry.foreign.borrow();
            let mut tabs = state.all_tabs();
            tabs.extend(state.bars());
            tabs.into_iter()
                .filter(|tab| registry.contains(*tab))
                .collect()
        };
        for tab in foreign {
            state.remove(tab);
            state.remove_bar(tab);
        }
        let keys = inner.keys.borrow();
        let mut tabs = state.all_tabs();
        tabs.extend(state.bars());
        tabs.extend(state.outside().iter().map(|outside| outside.tab));
        tabs.sort();
        tabs.dedup();
        let held: Vec<(K, TabId)> = tabs
            .into_iter()
            .filter_map(|tab| Some((keys.keys.get(&tab)?.clone(), tab)))
            .collect();
        DockingSnapshot { state, keys: held }
    }

    fn path(&self, host: TabId, key: &str) -> Option<String> {
        let mut path = key.to_owned();
        let mut host = host;
        for _ in 0..MAX_JOINED_DEPTH {
            let registry = &self.inner().registry;
            let Some((outer, inner)) = registry.foreign.borrow().inner(host) else {
                return Some(format!("{}/{path}", host.value()));
            };
            let joined = registry.joined(outer)?;
            let declaration = joined.declaration.borrow();
            path = format!("{}/{path}", declaration.as_ref()?.tab(inner)?.key);
            host = outer;
        }
        None
    }

    pub fn state(&self) -> ReadSignal<DockState> {
        self.state.read_only()
    }

    pub fn mode(&self) -> ReadSignal<DockMode> {
        self.mode.read_only()
    }

    pub fn tab_of(&self, key: &K) -> Option<TabId> {
        let inner = self.inner();
        inner.keyed.get();
        inner.keys.borrow().tabs.get(key).copied()
    }

    pub fn key_of(&self, tab: TabId) -> Option<K> {
        let inner = self.inner();
        inner.keyed.get();
        inner.keys.borrow().keys.get(&tab).cloned()
    }

    fn announce_keys(&self) {
        let inner = self.inner();
        let changed = std::mem::take(&mut inner.keys.borrow_mut().changed);
        if changed {
            inner.keyed.update(|keyed| *keyed += 1);
        }
    }

    pub fn contains(&self, key: &K) -> bool {
        let tab = self.tab_of(key);
        if let Some(held) = self.hosted(|hosted| tab.is_some_and(|tab| hosted.tabs.contains(&tab)))
        {
            return held;
        }
        self.state.with(|state| {
            tab.is_some_and(|tab| {
                state.contains(tab) || state.find_group(&joined_key(tab)).is_some()
            })
        })
    }

    pub fn focused(&self) -> Option<K> {
        let tab = match self.hosted(|hosted| hosted.focused) {
            Some(focused) => focused?,
            None => self.state.with(DockState::focused_tab)?,
        };
        self.key_of(self.outermost(tab))
    }

    pub fn shown(&self) -> Option<K> {
        let tab = match self.hosted(|hosted| hosted.shown) {
            Some(shown) => shown?,
            None => self.state.with(DockState::stacked_tab)?,
        };
        self.key_of(self.outermost(tab))
    }

    pub fn recent(&self) -> Vec<K> {
        let tabs = self.state.with(DockState::recent_tabs);
        let mut keys: Vec<K> = Vec::new();
        for tab in tabs {
            if let Some(key) = self.key_of(self.outermost(tab))
                && !keys.contains(&key)
            {
                keys.push(key);
            }
        }
        keys
    }

    pub fn area(&self) -> Rect {
        self.area.get()
    }

    pub fn set_covered(&self, covered: bool) {
        if self.covered.get_untracked() != covered {
            self.covered.set(covered);
        }
    }

    pub fn over_bars_rect(&self, edge: DockEdge, align: DockAlign) -> Option<Rect> {
        self.floats
            .with(|floats| floats.get(&(edge, align)).copied())
    }

    pub fn window_rect(&self, key: &K) -> Option<Rect> {
        let tab = self.tab_of(key)?;
        let rect = self.state.with(|state| {
            let surface = state.find(standing_in(state, tab))?.surface;
            state.window_rect(surface)
        })?;
        Some(rect.translate(self.area.get().min.to_vec2()))
    }

    pub fn move_window(&self, key: &K, to: Pos2) {
        let Some(tab) = self.tab_of(key) else {
            return;
        };
        let at = to - self.area.get_untracked().min.to_vec2();
        self.state.update(|state| {
            let Some(surface) = state
                .find(standing_in(state, tab))
                .map(|found| found.surface)
            else {
                return;
            };
            if let Some(rect) = state.window_rect(surface) {
                state.set_window_rect(surface, Rect::from_min_size(at, rect.size()));
            }
        });
    }

    fn outermost(&self, tab: TabId) -> TabId {
        let inner = self.inner();
        let foreign = inner.registry.foreign.borrow();
        let mut tab = tab;
        for _ in 0..MAX_JOINED_DEPTH {
            match foreign.inner(tab) {
                Some((host, _)) => tab = host,
                None => break,
            }
        }
        tab
    }

    pub fn show(&self, key: &K) {
        let Some(tab) = self.tab_of(key) else {
            return;
        };
        if let Some(host) = self.host() {
            host.request(DockRequest::Show(tab));
            return;
        }
        self.edit(|state| {
            let shown = standing_in(state, tab);
            state.show(shown);
        });
    }

    pub fn bar_placement(&self, key: &K) -> Option<DockBarPlacement> {
        let bar = self.tab_of(key)?;
        let own = self.state.with(|state| state.is_dock_bar(bar));
        let hosted = self.inner().bars_hosted.with(|hosted| {
            hosted
                .as_ref()
                .filter(|_| !own)
                .map(|hosted| hosted.state.bar_placement(bar))
        });
        match hosted {
            Some(placement) => placement,
            None => self.state.with(|state| state.bar_placement(bar)),
        }
    }

    pub fn place_bar(&self, key: &K, placement: DockBarPlacement) {
        let Some(bar) = self.tab_of(key) else {
            return;
        };
        let own = self.state.with_untracked(|state| state.is_dock_bar(bar));
        let bars_host = self.bars_host().filter(|_| !own);
        if let Some(host) = self.host().or(bars_host) {
            host.request(DockRequest::PlaceBar(bar, Some(placement)));
            return;
        }
        self.edit(|state| state.set_bar_placement(bar, placement));
    }

    pub fn is_bar_hidden(&self, key: &K) -> bool {
        let Some(bar) = self.tab_of(key) else {
            return false;
        };
        let own = self.state.with(|state| state.is_dock_bar(bar));
        let hosted = self.inner().bars_hosted.with(|hosted| {
            hosted
                .as_ref()
                .filter(|_| !own)
                .map(|hosted| hosted.state.is_bar_hidden(bar))
        });
        hosted.unwrap_or_else(|| self.state.with(|state| state.is_bar_hidden(bar)))
    }

    pub fn hide_bar(&self, key: &K) {
        self.set_bar_hidden(key, true);
    }

    pub fn show_bar(&self, key: &K) {
        self.set_bar_hidden(key, false);
    }

    fn set_bar_hidden(&self, key: &K, hidden: bool) {
        let Some(bar) = self.tab_of(key) else {
            return;
        };
        let own = self.state.with_untracked(|state| state.is_dock_bar(bar));
        let bars_host = self.bars_host().filter(|_| !own);
        if let Some(host) = self.host().or(bars_host) {
            host.request(DockRequest::HideBar(bar, hidden));
            return;
        }
        self.edit(|state| state.set_bar_hidden(bar, hidden));
    }

    fn bars_host(&self) -> Option<Rc<dyn DockHostLink>> {
        let inner = self.inner();
        let hosted = inner.bars_hosted.with_untracked(Option::is_some);
        inner.bars_link.borrow().clone().filter(|_| hosted)
    }

    fn declaration(&self, spec: DockSpec, found: Signal<Rc<Found<K>>>) -> DockDeclaration {
        let (mut tabs, mut dialogs) =
            found.with(|found| (found.declared_tabs(), found.declared_dialogs()));
        tabs.extend(found.with_untracked(|found| self.foreign_tabs(found)));
        let hosts: Vec<TabId> =
            found.with(|found| found.hosts.iter().map(|(host, _, _)| *host).collect());
        tabs.retain(|declared| !hosts.contains(&declared.tab));
        tabs.sort_by_key(|declared| declared.tab);
        dialogs.sort_by_key(|declared| declared.tab);
        let bars = found.with_untracked(|found| self.declared_bars(&spec, found));
        DockDeclaration {
            spec,
            tabs,
            dialogs,
            bars,
        }
    }

    pub fn fullscreen(&self) -> Option<K> {
        let tab = match self.hosted(|hosted| hosted.fullscreen) {
            Some(fullscreen) => fullscreen?,
            None => self.state.with(DockState::fullscreen)?.tab,
        };
        self.key_of(tab)
    }

    pub fn enter_fullscreen(&self, key: &K, area: Option<Rect>) {
        let Some(tab) = self.tab_of(key) else {
            return;
        };
        if let Some(host) = self.host() {
            host.request(DockRequest::Fullscreen(Some((tab, area))));
            return;
        }
        self.edit(|state| state.set_fullscreen(Some(DockFullscreen { tab, area })));
    }

    pub fn leave_fullscreen(&self) {
        if let Some(host) = self.host() {
            host.request(DockRequest::Fullscreen(None));
            return;
        }
        self.edit(|state| state.set_fullscreen(None));
    }

    fn edit(&self, change: impl FnOnce(&mut DockState)) {
        if self.host().is_some() {
            return;
        }
        let current = self.state.get_untracked();
        let mut next = current.clone();
        change(&mut next);
        if next != current {
            self.state.set(next);
        }
    }

    fn mint(&self, key: &K) -> TabId {
        let inner = self.inner();
        let mut keys = inner.keys.borrow_mut();
        if let Some(tab) = keys.tabs.get(key) {
            return *tab;
        }
        let named = key as &dyn Any;
        let named = named
            .downcast_ref::<TabId>()
            .copied()
            .or_else(|| named.downcast_ref::<u64>().copied().map(TabId::new))
            .filter(|tab| {
                !keys.keys.contains_key(tab) && !inner.registry.foreign.borrow().contains(*tab)
            });
        let tab = named.unwrap_or(TabId::new(keys.next));
        keys.next = keys.next.max(tab.value() + 1);
        keys.tabs.insert(key.clone(), tab);
        keys.keys.insert(tab, key.clone());
        keys.changed = true;
        tab
    }

    fn mint_foreign(&self, host: TabId, inner: TabId) -> TabId {
        let layout = self.inner();
        if let Some(tab) = layout.registry.foreign.borrow().outer(host, inner) {
            return tab;
        }
        let mut keys = layout.keys.borrow_mut();
        let mut tab = TabId::new(keys.next);
        while keys.keys.contains_key(&tab) {
            tab = TabId::new(tab.value() + 1);
        }
        keys.next = keys.next.max(tab.value() + 1);
        keys.changed = true;
        layout
            .registry
            .foreign
            .borrow_mut()
            .insert(host, inner, tab);
        tab
    }

    fn reconcile(
        &self,
        spec: &DockSpec,
        dialogs: &[(TabId, TabId)],
        hosts: &[(TabId, DockSpecNode, Option<TabId>)],
        bar_key: &dyn Fn(TabId) -> String,
    ) {
        let current = self.state.get_untracked();
        let mut next = current.clone();
        let live: HashSet<TabId> = hosts.iter().map(|(host, _, _)| *host).collect();
        while let Some((group, host)) = next
            .joined_groups()
            .into_iter()
            .find(|(_, host)| !live.contains(host))
        {
            next.unhost_group(group, host);
        }
        for (host, root, focus) in hosts {
            let key = joined_key(*host);
            if next.find_group(&key).is_none() && next.contains(*host) {
                next.host_group(*host, &key, root, *focus);
            }
        }
        next.reconcile(spec);
        next.adopt_bar_placements(&current, bar_key);
        let registry = Rc::clone(&self.inner().registry);
        for (host, _, _) in hosts {
            let pending = registry
                .joined(*host)
                .and_then(|joined| joined.pending.borrow_mut().take());
            if let Some(pending) = pending {
                registry.graft(&mut next, *host, &pending);
            }
        }
        let mut kept: HashSet<TabId> = spec.tabs().into_iter().collect();
        kept.extend(next.all_tabs());
        kept.extend(live);
        kept.extend(
            dialogs
                .iter()
                .flat_map(|(dialog, parent)| [*dialog, *parent]),
        );
        kept.extend(
            spec.bars
                .iter()
                .flat_map(|bar| std::iter::once(bar.bar).chain(bar.owner)),
        );
        self.retain_keys(&kept);
        registry
            .foreign
            .borrow_mut()
            .retain(|tab| kept.contains(&tab));
        if next != current {
            self.state.set(next);
        }
        self.announce_keys();
    }

    fn retain_keys(&self, kept: &HashSet<TabId>) {
        let inner = self.inner();
        let mut keys = inner.keys.borrow_mut();
        let held = keys.keys.len();
        keys.keys.retain(|tab, _| kept.contains(tab));
        keys.tabs.retain(|_, tab| kept.contains(tab));
        keys.changed |= keys.keys.len() != held;
    }

    fn declared(&self, spec: &DockSpec, dialogs: &[(TabId, TabId)], hosts: &[TabId]) {
        let mut kept: HashSet<TabId> = spec.tabs().into_iter().collect();
        kept.extend(
            self.hosted(|hosted| hosted.tabs.clone())
                .unwrap_or_default(),
        );
        kept.extend(
            dialogs
                .iter()
                .flat_map(|(dialog, parent)| [*dialog, *parent]),
        );
        kept.extend(hosts.iter().copied());
        kept.extend(
            spec.bars
                .iter()
                .flat_map(|bar| std::iter::once(bar.bar).chain(bar.owner)),
        );
        self.retain_keys(&kept);
        self.inner()
            .registry
            .foreign
            .borrow_mut()
            .retain(|tab| kept.contains(&tab));
        *self.inner().declared.borrow_mut() = Some(spec.clone());
        self.announce_keys();
    }

    fn foreign_tabs(&self, found: &Found<K>) -> Vec<DockDeclaredTab> {
        let inner = self.inner();
        found
            .foreign
            .iter()
            .filter_map(|(tab, (joined, held))| {
                let declaration = joined.declaration.borrow();
                let declared = declaration.as_ref()?.tab(*held)?;
                let (host, _) = inner.registry.foreign.borrow().inner(*tab)?;
                Some(DockDeclaredTab {
                    tab: *tab,
                    key: self.path(host, &declared.key)?,
                    ..declared.clone()
                })
            })
            .collect()
    }

    fn declared_bars(&self, spec: &DockSpec, found: &Found<K>) -> Vec<DockDeclaredBar> {
        let inner = self.inner();
        spec.bars
            .iter()
            .filter_map(|bar| {
                let (key, kind) = match found.bars.get(&bar.bar) {
                    Some((key, _)) => (key.clone(), key.clone()),
                    None => {
                        let (joined, held) = found.foreign.get(&bar.bar)?;
                        let declaration = joined.declaration.borrow();
                        let declared = declaration.as_ref()?.bar(*held)?;
                        let (host, _) = inner.registry.foreign.borrow().inner(bar.bar)?;
                        (self.path(host, &declared.key)?, declared.kind.clone())
                    }
                };
                Some(DockDeclaredBar {
                    tab: bar.bar,
                    key,
                    kind,
                    owner: bar.owner,
                    placement: bar.placement,
                })
            })
            .collect()
    }

    fn foreign_dialog(
        &self,
        tab: TabId,
        joined: &Rc<Joined>,
        inner: TabId,
        declared: &DockDeclaredDialog,
    ) -> Option<DialogParts> {
        let layout = self.inner();
        if let Some(held) = layout.dialogs.borrow().get(&tab) {
            if held
                .described
                .with_untracked(|described| described != declared)
            {
                held.described.set(declared.clone());
            }
            return Some(held.parts);
        }
        let scope = layout._scope.context().child()?;
        let stored = self.inner;
        let closing = Rc::clone(joined);
        let closable = declared.closable;
        let (described, parts) = scope.run(|| {
            let described = create_signal(declared.clone());
            let read = move |read: fn(&DockDeclaredDialog) -> bool| {
                create_memo(move || described.with(read))
            };
            let parts = DialogParts {
                title: create_memo(move || described.with(|described| described.title.clone())),
                icon: create_memo(move || described.with(|described| described.icon)),
                size: create_memo(move || described.with(|described| described.size)),
                modal: read(|described| described.modal),
                dismiss_on_outside_press: read(|described| described.dismiss_on_outside_press),
                on_close: closable.then(|| ClickCallback::new(move || closing.close(inner))),
                content: RenderFn::new(move |()| {
                    let place = stored
                        .value()
                        .place
                        .get()
                        .expect("the Docking that holds a dialog places it");
                    place.call(tab)
                }),
            };
            (described, parts)
        });
        layout.dialogs.borrow_mut().insert(
            tab,
            ForeignDialog {
                described,
                parts,
                _scope: scope,
            },
        );
        Some(parts)
    }

    fn mirror(&self, hosted: Option<DockHosted>) {
        let inner = self.inner();
        if let Some(hosted) = &hosted {
            if self.state.with_untracked(|state| *state != hosted.state) {
                self.state.set(hosted.state.clone());
            }
            if self.mode.get_untracked() != hosted.mode {
                self.mode.set(hosted.mode);
            }
        }
        inner.hosted.set(hosted);
    }

    fn spec(&self, nodes: &[Rc<DockNode<K>>], focus: Option<&K>, found: &mut Found<K>) -> DockSpec {
        let mut spec = DockSpec::default();
        for node in nodes {
            if let NodeKind::Dialog { id, parent, parts } = &node.kind {
                let dialog = self.mint(id);
                if found.dialogs.iter().any(|(held, _)| *held == dialog) {
                    continue;
                }
                let parent = self.mint(parent);
                found.dialogs.push((dialog, parent));
                found.parts.insert(dialog, *parts);
            }
            if let NodeKind::Bar {
                id,
                parent,
                placement,
                content,
            } = &node.kind
            {
                let bar = self.mint(id);
                if found.bars.contains_key(&bar) {
                    continue;
                }
                let owner = parent.as_ref().map(|parent| self.mint(parent));
                found.bars.insert(bar, (id.key(), *content));
                found.spec_bars.push(DockSpecBar {
                    bar,
                    owner,
                    placement: *placement,
                });
            }
        }
        for node in nodes {
            match &node.kind {
                NodeKind::Window {
                    key,
                    rect,
                    children,
                } => spec.windows.push(DockSpecWindow {
                    key: key.clone(),
                    rect: *rect,
                    root: self.root(children, found),
                }),
                NodeKind::Dialog { .. } | NodeKind::Bar { .. } => {}
                _ if spec.main.is_none() => spec.main = self.node(node, found),
                _ => {}
            }
        }
        spec.windows.append(&mut found.windows);
        spec.bars = found.spec_bars.clone();
        spec.focus = focus.map(|key| self.mint(key));
        spec.see_through = found
            .tabs
            .iter()
            .filter(|(_, entry)| {
                matches!(
                    entry.kind,
                    EntryKind::Tab {
                        see_through: true,
                        ..
                    }
                )
            })
            .map(|(tab, _)| *tab)
            .collect();
        spec.see_through.sort();
        spec
    }

    fn root(&self, children: &Run<DockNode<K>>, found: &mut Found<K>) -> Option<DockSpecNode> {
        children
            .items()
            .iter()
            .filter(|node| {
                !matches!(
                    node.kind,
                    NodeKind::Window { .. } | NodeKind::Dialog { .. } | NodeKind::Bar { .. }
                )
            })
            .find_map(|node| self.node(node, found))
    }

    fn node(&self, node: &Rc<DockNode<K>>, found: &mut Found<K>) -> Option<DockSpecNode> {
        match &node.kind {
            NodeKind::Split {
                key,
                direction,
                fraction,
                children,
            } => {
                let mut parts: Vec<DockSpecNode> = children
                    .items()
                    .iter()
                    .filter_map(|child| self.node(child, found))
                    .collect();
                if parts.len() < 2 {
                    return parts.pop();
                }
                let second = parts.swap_remove(1);
                let first = parts.swap_remove(0);
                Some(DockSpecNode::Split {
                    key: key.clone(),
                    direction: *direction,
                    fraction: fraction.get(),
                    first: Box::new(first),
                    second: Box::new(second),
                })
            }
            NodeKind::Pane {
                key,
                active,
                vertical,
                icons_only,
                sidebar,
                locked,
                bare,
                empty,
                entries,
            } => {
                if empty.is_some() {
                    found.panes.insert(key.clone(), Rc::clone(node));
                }
                let entries = entries
                    .items()
                    .iter()
                    .filter_map(|entry| self.entry(entry, found))
                    .collect();
                Some(DockSpecNode::Pane(DockSpecPane {
                    key: key.clone(),
                    entries,
                    active: active.as_ref().map(|key| self.mint(key)),
                    vertical: *vertical,
                    icons_only: *icons_only,
                    sidebar: *sidebar,
                    keep: empty.is_some() || *bare,
                    locked: *locked,
                    bare: *bare,
                }))
            }
            NodeKind::Window { .. } | NodeKind::Dialog { .. } | NodeKind::Bar { .. } => None,
        }
    }

    fn entry(&self, entry: &Rc<DockEntry<K>>, found: &mut Found<K>) -> Option<DockSpecEntry> {
        match &entry.kind {
            EntryKind::Tab { id, .. } => {
                let tab = self.mint(id);
                if found.parts.contains_key(&tab) {
                    return None;
                }
                if found.tabs.insert(tab, Rc::clone(entry)).is_some() {
                    return None;
                }
                Some(self.splice(tab, found).unwrap_or(DockSpecEntry::Tab(tab)))
            }
            EntryKind::Group {
                key,
                pinned,
                children,
                ..
            } => {
                found.groups.insert(key.clone(), Rc::clone(entry));
                let root = self.root(children, found)?;
                Some(DockSpecEntry::Group {
                    key: key.clone(),
                    pinned: *pinned,
                    root: Box::new(root),
                })
            }
        }
    }
}

impl<K: DockKey> DockingLayout<K> {
    fn splice(&self, host: TabId, found: &mut Found<K>) -> Option<DockSpecEntry> {
        let registry = Rc::clone(&self.inner().registry);
        let joined = registry.joined(host)?;
        let spec = joined.declaration.borrow().as_ref()?.spec.clone();
        let root = spec.main.as_ref()?;
        let root = self.foreign_node(host, &joined, root, found);
        let focus = spec.focus.map(|inner| self.mint_foreign(host, inner));
        let dialogs = joined
            .declaration
            .borrow()
            .as_ref()
            .map(|declaration| declaration.dialogs.clone())
            .unwrap_or_default();
        for dialog in &dialogs {
            let tab = self.mint_foreign(host, dialog.tab);
            let parent = self.mint_foreign(host, dialog.parent);
            if found.dialogs.iter().any(|(held, _)| *held == tab) {
                continue;
            }
            let Some(parts) = self.foreign_dialog(tab, &joined, dialog.tab, dialog) else {
                continue;
            };
            found.foreign.insert(tab, (Rc::clone(&joined), dialog.tab));
            found.dialogs.push((tab, parent));
            found.parts.insert(tab, parts);
        }
        let declared_bars = joined
            .declaration
            .borrow()
            .as_ref()
            .map(|declaration| declaration.bars.clone())
            .unwrap_or_default();
        for declared in &declared_bars {
            let bar = self.mint_foreign(host, declared.tab);
            let owner = declared
                .owner
                .map_or(host, |owner| self.mint_foreign(host, owner));
            if found.spec_bars.iter().any(|held| held.bar == bar) {
                continue;
            }
            found
                .foreign
                .insert(bar, (Rc::clone(&joined), declared.tab));
            found.spec_bars.push(DockSpecBar {
                bar,
                owner: Some(owner),
                placement: declared.placement,
            });
        }
        for window in &spec.windows {
            let root = window
                .root
                .as_ref()
                .map(|root| self.foreign_node(host, &joined, root, found));
            found.windows.push(DockSpecWindow {
                key: format!("{}{}", joined_prefix(host), window.key),
                rect: window.rect,
                root,
            });
        }
        found.hosts.push((host, root.clone(), focus));
        Some(DockSpecEntry::Group {
            key: joined_key(host),
            pinned: true,
            root: Box::new(root),
        })
    }

    fn foreign_node(
        &self,
        host: TabId,
        joined: &Rc<Joined>,
        node: &DockSpecNode,
        found: &mut Found<K>,
    ) -> DockSpecNode {
        let prefix = joined_prefix(host);
        match node {
            DockSpecNode::Split {
                key,
                direction,
                fraction,
                first,
                second,
            } => DockSpecNode::Split {
                key: format!("{prefix}{key}"),
                direction: *direction,
                fraction: *fraction,
                first: Box::new(self.foreign_node(host, joined, first, found)),
                second: Box::new(self.foreign_node(host, joined, second, found)),
            },
            DockSpecNode::Pane(pane) => {
                let entries = pane
                    .entries
                    .iter()
                    .map(|entry| match entry {
                        DockSpecEntry::Tab(inner) => {
                            let tab = self.mint_foreign(host, *inner);
                            found.foreign.insert(tab, (Rc::clone(joined), *inner));
                            self.splice(tab, found).unwrap_or(DockSpecEntry::Tab(tab))
                        }
                        DockSpecEntry::Group { key, pinned, root } => DockSpecEntry::Group {
                            key: format!("{prefix}{key}"),
                            pinned: *pinned,
                            root: Box::new(self.foreign_node(host, joined, root, found)),
                        },
                    })
                    .collect();
                DockSpecNode::Pane(DockSpecPane {
                    key: format!("{prefix}{}", pane.key),
                    entries,
                    active: pane.active.map(|inner| self.mint_foreign(host, inner)),
                    ..pane.clone()
                })
            }
        }
    }
}

struct Found<K: 'static> {
    foreign: HashMap<TabId, (Rc<Joined>, TabId)>,
    hosts: Vec<(TabId, DockSpecNode, Option<TabId>)>,
    windows: Vec<DockSpecWindow>,
    tabs: HashMap<TabId, Rc<DockEntry<K>>>,
    panes: HashMap<String, Rc<DockNode<K>>>,
    groups: HashMap<String, Rc<DockEntry<K>>>,
    dialogs: Vec<(TabId, TabId)>,
    parts: HashMap<TabId, DialogParts>,
    bars: HashMap<TabId, (String, RenderFn<()>)>,
    spec_bars: Vec<DockSpecBar>,
}

impl<K: 'static> Default for Found<K> {
    fn default() -> Self {
        Self {
            bars: HashMap::default(),
            spec_bars: Vec::new(),
            foreign: HashMap::default(),
            hosts: Vec::new(),
            windows: Vec::new(),
            tabs: HashMap::default(),
            panes: HashMap::default(),
            groups: HashMap::default(),
            dialogs: Vec::new(),
            parts: HashMap::default(),
        }
    }
}

impl<K: 'static> PartialEq for Found<K> {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

impl<K: DockKey> Found<K> {
    fn declared_tabs(&self) -> Vec<DockDeclaredTab> {
        self.tabs
            .iter()
            .filter_map(|(tab, entry)| match &entry.kind {
                EntryKind::Tab {
                    id,
                    title,
                    icon,
                    picture,
                    on_close,
                    ..
                } => Some(DockDeclaredTab {
                    tab: *tab,
                    key: id.key(),
                    title: title.get(),
                    icon: icon.get(),
                    picture: picture.get(),
                    closable: on_close.is_some(),
                }),
                EntryKind::Group { .. } => None,
            })
            .collect()
    }

    fn bar_key(&self, bar: TabId) -> Option<String> {
        if let Some((key, _)) = self.bars.get(&bar) {
            return Some(key.clone());
        }
        let (joined, inner) = self.foreign.get(&bar)?;
        let declaration = joined.declaration.borrow();
        Some(declaration.as_ref()?.bar(*inner)?.kind.clone())
    }

    fn own_keys(&self) -> Vec<(TabId, String)> {
        let mut keys: Vec<(TabId, String)> = self
            .tabs
            .iter()
            .filter_map(|(tab, entry)| match &entry.kind {
                EntryKind::Tab { id, .. } => Some((*tab, id.key())),
                EntryKind::Group { .. } => None,
            })
            .chain(self.bars.iter().map(|(bar, (key, _))| (*bar, key.clone())))
            .collect();
        keys.sort();
        keys
    }

    fn declared_dialogs(&self) -> Vec<DockDeclaredDialog> {
        self.dialogs
            .iter()
            .filter_map(|(tab, parent)| {
                let parts = self.parts.get(tab)?;
                Some(DockDeclaredDialog {
                    tab: *tab,
                    parent: *parent,
                    title: parts.title.get(),
                    icon: parts.icon.get(),
                    size: parts.size.get(),
                    modal: parts.modal.get(),
                    dismiss_on_outside_press: parts.dismiss_on_outside_press.get(),
                    closable: parts.on_close.is_some(),
                })
            })
            .collect()
    }

    fn tab<R>(
        &self,
        tab: TabId,
        read: impl FnOnce(
            &Memo<String>,
            &Memo<Option<Glyph>>,
            &Option<ClickCallback>,
            &RenderFn<()>,
        ) -> R,
    ) -> Option<R> {
        match &self.tabs.get(&tab)?.kind {
            EntryKind::Tab {
                title,
                icon,
                on_close,
                content,
                ..
            } => Some(read(title, icon, on_close, content)),
            EntryKind::Group { .. } => None,
        }
    }

    fn see_through(&self, tab: TabId) -> bool {
        self.tabs.get(&tab).is_some_and(|entry| {
            matches!(
                entry.kind,
                EntryKind::Tab {
                    see_through: true,
                    ..
                }
            )
        })
    }

    fn picture(&self, tab: TabId) -> Option<Image> {
        match &self.tabs.get(&tab)?.kind {
            EntryKind::Tab { picture, .. } => picture.is_alive().then(|| picture.get()).flatten(),
            EntryKind::Group { .. } => None,
        }
    }

    fn empty(&self, key: &str) -> Option<RenderFn<()>> {
        match &self.panes.get(key)?.kind {
            NodeKind::Pane { empty, .. } => *empty,
            _ => None,
        }
    }

    fn group_title(&self, key: &str) -> Option<Memo<String>> {
        match &self.groups.get(key)?.kind {
            EntryKind::Group { title, .. } => *title,
            EntryKind::Tab { .. } => None,
        }
    }
}

#[component]
pub fn Docking<K>(
    #[prop(default = ComponentRef::new())] api: ComponentRef<DockApi>,
    layout: DockingLayout<K>,
    #[prop(default = 0.0)] stack_below: f32,
    #[prop(default = None)] focus: Prop<Option<K>>,
    #[prop(default = None)] drag_modifier: Prop<Option<Modifiers>>,
    menu: MenuStyle,
    splitter_thickness: f32,
    #[prop(default = 0.0)] splitter_reach: f32,
    #[prop(default = 0.0)] group_inset: f32,
    tab_spacing: f32,
    drop_marker_width: f32,
    back_shade: Prop<Color>,
    icon_sidebar_width: f32,
    #[prop(default = 0.0)] inset: Prop<f32>,
    tab: RenderFn<DockTabHandle>,
    frame: Option<RenderFn<NodeId>>,
    fullscreen: Option<RenderFn<NodeId>>,
    away: Option<RenderFn<DockAwayHandle>>,
    chrome: Option<RenderFn<DockChromeHandle>>,
    bar: Option<RenderFn<DockBarHandle>>,
    splitter: Option<RenderFn<DockSplitterHandle>>,
    strip: Option<RenderFn<DockStripHandle>>,
    grip: Option<RenderFn<DockGripHandle>>,
    highlight: Option<RenderFn<()>>,
    refused: Option<RenderFn<()>>,
    preview: Option<RenderFn<DockPreviewHandle>>,
    stack: Option<RenderFn<DockStackHandle>>,
    switcher: Option<RenderFn<DockSwitcherHandle>>,
    switch: Option<RenderFn<DockSwitchHandle>>,
    dialog_chrome: Option<RenderFn<DockDialogHandle>>,
    #[prop(default = 0.0)] dialog_beak: f32,
    #[prop(default = 0.0)] dialog_radius: f32,
    #[prop(default = Color::TRANSPARENT)] dialog_scrim: Prop<Color>,
    #[prop(default = 0.0)] window_border: f32,
    #[prop(default = 0.0)] bar_margin: f32,
    bar_band: Option<RenderFn<DockBarBandHandle>>,
    children: Children<DockNode<K>>,
) -> NodeId
where
    K: DockKey,
{
    let nodes = children.into_run();
    let found = create_signal(Rc::new(Found::<K>::default()));
    let on_close = move |tab: TabId| {
        let close = found.with_untracked(|found| match found.foreign.get(&tab) {
            Some((joined, inner)) => Err((Rc::clone(joined), *inner)),
            None => Ok(found
                .tab(tab, |_, _, on_close, _| *on_close)
                .flatten()
                .or_else(|| found.parts.get(&tab).and_then(|parts| parts.on_close))),
        });
        match close {
            Ok(Some(close)) => close.call(),
            Ok(None) => {}
            Err((joined, inner)) => joined.close(inner),
        }
    };
    let body = RenderFn::new(move |tab: TabId| {
        if let Some(bar) = found.with_untracked(|found| found.bars.get(&tab).map(|(_, bar)| *bar)) {
            return bar.call(());
        }
        let content = found.with_untracked(|found| found.tab(tab, |_, _, _, content| *content));
        let Some(content) = content else {
            let dialog =
                found.with_untracked(|found| found.parts.get(&tab).map(|parts| parts.content));
            return match dialog {
                Some(dialog) => dialog.call(()),
                None => view! {
                    <Frame />
                },
            };
        };
        let see_through = found.with_untracked(|found| found.see_through(tab));
        let body = content.call(());
        match &frame {
            Some(frame) if !see_through => frame.call(body),
            _ => body,
        }
    });
    let slot = use_context::<DockHostSlot>();
    let link = match use_dock_join().and_then(|join| join.claim()) {
        Some(link) => Some((link, false)),
        None => slot
            .as_ref()
            .and_then(|slot| slot.claim())
            .map(|link| (link, true)),
    };
    let (link, bars) = match link {
        Some((link, true)) if slot.as_ref().is_some_and(DockHostSlot::joins_bars) => {
            (None, Some(link))
        }
        link => (link, None),
    };
    let keeper = match link {
        Some((_, false)) => None,
        _ => use_context::<DockKeeper>().and_then(|keeper| keeper.claim()),
    };
    if let Some((link, nests)) = link {
        let on_close = Callback::new(on_close);
        return view! {
            <JoinedDock layout link nests nodes found body focus on_close />
        };
    }
    let registry = Rc::clone(&layout.inner().registry);
    registry.dock.set(Some(api));
    let version = registry.version;
    let declared = create_signal(None::<DockSpec>);
    let declares = bars.is_some();
    create_effect(move || {
        layout.restores.get();
        version.get();
        let items = nodes.items();
        let mut held = Found::default();
        let focus = focus.get_untracked();
        let spec = layout.spec(&items, focus.as_ref(), &mut held);
        let dialogs = held.dialogs.clone();
        let hosts = held.hosts.clone();
        untrack(|| {
            found.set(Rc::new(held));
            let bar_key =
                |bar: TabId| shared_key(found.with_untracked(|found| found.bar_key(bar)), bar);
            layout.reconcile(&spec, &dialogs, &hosts, &bar_key);
            if declares {
                declared.set(Some(spec));
            }
        });
    });
    let bars_hosted = layout.inner().bars_hosted;
    if let Some(link) = bars {
        join_bars(layout, link, found, declared.read_only());
    }
    if let Some(keep) = keeper {
        keep(DockKept {
            layout: create_memo(move || layout.snapshot().state),
            keys: create_memo(move || found.with(|found| found.own_keys())),
            restore: Callback::new(move |state: DockState| layout.keep_state(state)),
        });
    }
    let dialogs = create_memo(move || {
        let held = found.with(|found| found.dialogs.clone());
        layout.state.with(|state| {
            held.into_iter()
                .map(|(dialog, parent)| (dialog, standing_in(state, parent)))
                .collect::<Vec<_>>()
        })
    });
    let dialog =
        Func::new(move |tab: TabId| found.with_untracked(|found| found.parts.get(&tab).cloned()));
    let declared = move |tab: TabId| -> Option<DockDeclaredTab> {
        found.with(|found| {
            let (joined, inner) = found.foreign.get(&tab)?;
            joined.declaration.borrow().as_ref()?.tab(*inner).cloned()
        })
    };
    let title = Func::new(move |tab: TabId| {
        if let Some(declared) = declared(tab) {
            return declared.title;
        }
        found
            .with(|found| found.tab(tab, |title, _, _, _| title.is_alive().then(|| title.get())))
            .flatten()
            .unwrap_or_default()
    });
    let icon = Func::new(move |tab: TabId| {
        if let Some(declared) = declared(tab) {
            return declared.icon;
        }
        found
            .with(|found| found.tab(tab, |_, icon, _, _| icon.is_alive().then(|| icon.get())))
            .flatten()
            .unwrap_or_default()
    });
    let picture = Func::new(move |tab: TabId| {
        if let Some(declared) = declared(tab) {
            return declared.picture;
        }
        found.with(|found| found.picture(tab))
    });
    let closable = Func::new(move |tab: TabId| {
        if let Some(declared) = declared(tab) {
            return declared.closable;
        }
        found
            .with(|found| found.tab(tab, |_, _, on_close, _| on_close.is_some()))
            .unwrap_or(false)
    });
    let place = RenderFn::new(move |tab: TabId| {
        let join = |host: TabId| DockJoin {
            host,
            registry: Rc::clone(&registry),
            claimed: Rc::default(),
        };
        let foreign = found.with_untracked(|found| found.foreign.get(&tab).cloned());
        let Some((joined, inner)) = foreign else {
            provide_context(join(tab));
            let node = body.call(tab);
            if declares {
                let covers = create_memo(move || {
                    bars_hosted.with(|hosted| {
                        hosted.as_ref().map_or_else(Vec::new, |hosted| {
                            hosted
                                .covers
                                .iter()
                                .filter(|(covered, _)| *covered == tab)
                                .map(|(_, rect)| *rect)
                                .collect::<Vec<_>>()
                        })
                    })
                });
                keep_clear(node, covers);
            }
            return node;
        };
        place_foreign(&registry, &joined, tab, inner).unwrap_or_else(|| {
            view! {
                <Frame />
            }
        })
    });
    layout.inner().place.set(Some(place));
    let content = RenderFn::new(move |tab: TabId| {
        let known = move |found: &Found<K>| {
            found.foreign.contains_key(&tab)
                || found.tabs.contains_key(&tab)
                || found.bars.contains_key(&tab)
        };
        if found.with_untracked(|found| known(found)) {
            return place.call(tab);
        }
        let declared = create_memo(move || found.with(|found| known(found)));
        view! {
            <Layers>
                <Dynamic value={declared}>|declared: bool| {
                    match declared {
                        true => place.call(tab),
                        false => view! {
                            <Frame />
                        },
                    }
                }</Dynamic>
            </Layers>
        }
    });
    let empty = RenderFn::new(move |leaf: LeafId| {
        let key = layout
            .state
            .with_untracked(|state| state.pane_key(leaf).map(str::to_owned));
        let joined = key.as_deref().and_then(|key| {
            let (host, rest) = key.strip_prefix("joined:")?.split_once('/')?;
            let host = TabId::new(host.parse().ok()?);
            let joined = layout.inner().registry.joined(host)?;
            let bodies = joined.bodies.borrow().clone()?;
            bodies.empty.call(rest.to_owned())
        });
        if let Some(joined) = joined {
            return joined;
        }
        let Some(key) = key else {
            return view! {
                <Frame />
            };
        };
        if let Some(empty) = found.with_untracked(|found| found.empty(&key)) {
            return empty.call(());
        }
        let declared = {
            let key = key.clone();
            create_memo(move || found.with(|found| found.empty(&key).is_some()))
        };
        view! {
            <Layers>
                <Dynamic value={declared}>|_: bool| {
                    match found.with_untracked(|found| found.empty(&key)) {
                        Some(empty) => empty.call(()),
                        None => view! {
                            <Frame />
                        },
                    }
                }</Dynamic>
            </Layers>
        }
    });
    let group_host = Func::new(move |group: GroupId| {
        layout
            .state
            .with(|state| state.group_key(group).and_then(joined_host))
    });
    let group_title = Func::new(move |group: GroupId| {
        let key = layout
            .state
            .with(|state| state.group_key(group).map(str::to_owned))?;
        if let Some(host) = joined_host(&key) {
            return Some(title.call(host));
        }
        let title = found.with(|found| found.group_title(&key))?;
        Some(title.get())
    });
    let hosts = create_memo(move || {
        found.with(|found| {
            found
                .hosts
                .iter()
                .map(|(host, _, _)| *host)
                .collect::<Vec<_>>()
        })
    });
    view! {
        <Dock
            config={DockConfig {
                api,
                state: Binding::from(layout.state),
                on_close: Callback::new(on_close),
                title,
                group_title,
                group_host,
                hosts,
                icon,
                picture,
                closable,
                menu,
                stack_below,
                mode: layout.mode,
                drag_modifier,
                splitter_thickness,
                splitter_reach,
                group_inset,
                tab_spacing,
                marker_width: drop_marker_width,
                back_shade: create_memo(move || back_shade.get()),
                icon_sidebar_width,
                inset,
                tab,
                content,
                empty,
                fullscreen,
                away,
                chrome,
                bar,
                splitter,
                strip,
                grip,
                highlight,
                refused,
                preview,
                stack,
                switcher,
                switch,
                dialogs,
                dialog,
                dialog_chrome,
                dialog_beak,
                dialog_radius,
                dialog_scrim: create_memo(move || dialog_scrim.get()),
                window_border,
                bar_margin,
                bar_band,
                covers: layout.covers,
                floats: layout.floats,
                covered: layout.covered,
                area: layout.area,
                bars_away: create_memo(move || bars_hosted.with(Option::is_some)),
                bar_key: Func::new(move |bar: TabId| found.with(|found| found.bar_key(bar))),
            }}
        />
    }
}

fn join_bars<K: DockKey>(
    layout: DockingLayout<K>,
    link: Rc<dyn DockHostLink>,
    found: Signal<Rc<Found<K>>>,
    declared: ReadSignal<Option<DockSpec>>,
) {
    let inner = layout.inner();
    *inner.bars_link.borrow_mut() = Some(Rc::clone(&link));
    let hosted = link.hosted();
    let held = inner.bars_hosted;
    create_effect(move || {
        let hosted = hosted.get();
        untrack(|| held.set(hosted));
    });
    let declaring = Rc::clone(&link);
    create_effect(move || {
        let Some(mut spec) = declared.get() else {
            return;
        };
        spec.bars.retain(|bar| bar.owner.is_some());
        if spec.bars.is_empty() {
            return;
        }
        untrack(|| declaring.declare(layout.declaration(spec, found)));
    });
    let body = RenderFn::new(move |bar: TabId| {
        let body = found.with_untracked(|found| found.bars.get(&bar).map(|(_, body)| *body));
        match body {
            Some(body) => body.call(()),
            None => view! {
                <Frame />
            },
        }
    });
    if let Some(scope) = owner_scope() {
        link.place(DockBodies {
            scope,
            body,
            empty: RenderFn::new(|_: String| None),
            hosts: create_memo(Vec::new),
        });
    }
    let kept = layout.inner;
    on_cleanup(move || {
        link.withdraw();
        if let Some(inner) = kept.try_value() {
            inner.bars_link.borrow_mut().take();
            if inner.bars_hosted.is_alive() {
                inner.bars_hosted.set(None);
            }
        }
    });
}

fn standing_in(state: &DockState, tab: TabId) -> TabId {
    if state.contains(tab) {
        return tab;
    }
    let Some(group) = state.find_group(&joined_key(tab)) else {
        return tab;
    };
    let inside = state.group_tabs(group);
    state
        .recent_tabs()
        .into_iter()
        .find(|recent| inside.contains(recent))
        .or_else(|| inside.first().copied())
        .unwrap_or(tab)
}

fn place_foreign(
    registry: &Rc<Registry>,
    joined: &Rc<Joined>,
    tab: TabId,
    inner: TabId,
) -> Option<NodeId> {
    let bodies = joined.bodies.borrow().clone();
    let scope = bodies.as_ref().and_then(|bodies| bodies.scope.child())?;
    let control = use_context::<DockTabControl>();
    let bar = use_context::<DockBarControl>();
    let menu = use_context::<DockTabMenu>();
    let layout = use_context::<DialogLayout>();
    let registry = Rc::clone(registry);
    let placed = scope.run(|| {
        if let Some(control) = control {
            provide_context(control);
        }
        if let Some(bar) = bar {
            provide_context(bar);
        }
        if let Some(menu) = menu {
            provide_context(menu);
        }
        if let Some(layout) = layout {
            provide_context(layout);
        }
        provide_context(DockJoin {
            host: tab,
            registry,
            claimed: Rc::default(),
        });
        bodies.map(|bodies| bodies.body.call(inner))
    });
    on_cleanup(move || drop(scope));
    placed
}

#[component]
fn JoinedDock<K>(
    layout: DockingLayout<K>,
    link: Rc<dyn DockHostLink>,
    nests: bool,
    nodes: Run<DockNode<K>>,
    found: Signal<Rc<Found<K>>>,
    body: RenderFn<TabId, NodeId>,
    focus: Prop<Option<K>>,
    on_close: Callback<TabId>,
) -> NodeId
where
    K: DockKey,
{
    *layout.inner().host.borrow_mut() = Some(Rc::clone(&link));
    let registry = Rc::clone(&layout.inner().registry);
    *registry.host.borrow_mut() = Some(Rc::clone(&link));
    if layout.state.with_untracked(DockState::is_seeded) {
        link.request(DockRequest::Restore(layout.state.get_untracked()));
    }
    let hosted = link.hosted();
    create_effect(move || {
        let hosted = hosted.get();
        untrack(|| layout.mirror(hosted));
    });
    let spec = create_signal(None::<DockSpec>);
    let version = registry.version;
    create_effect(move || {
        layout.restores.get();
        if nests {
            version.get();
        }
        let items = nodes.items();
        let mut held = Found::default();
        let focus = focus.get_untracked();
        let declared = layout.spec(&items, focus.as_ref(), &mut held);
        let dialogs = held.dialogs.clone();
        let hosts: Vec<TabId> = held.hosts.iter().map(|(host, _, _)| *host).collect();
        untrack(|| {
            found.set(Rc::new(held));
            layout.declared(&declared, &dialogs, &hosts);
            spec.set(Some(declared));
        });
    });
    let declaring = Rc::clone(&link);
    create_effect(move || {
        let Some(spec) = spec.get() else {
            return;
        };
        let declaration = layout.declaration(spec, found);
        untrack(|| declaring.declare(declaration));
    });
    link.listen(Callback::new(move |event: DockHostEvent| match event {
        DockHostEvent::Close(tab) => on_close.call(tab),
    }));
    let emptying = Rc::clone(&registry);
    let empty = RenderFn::new(move |key: String| {
        if let Some((host, rest)) = key.split_once('/')
            && let Some(host) = joined_host(host)
        {
            let bodies = emptying.joined(host)?.bodies.borrow().clone()?;
            return bodies.empty.call(rest.to_owned());
        }
        found
            .with_untracked(|found| found.empty(&key))
            .map(|empty| empty.call(()))
    });
    let placing = Rc::clone(&registry);
    let body = match nests {
        false => body,
        true => RenderFn::new(move |tab: TabId| {
            let foreign = found.with_untracked(|found| found.foreign.get(&tab).cloned());
            match foreign {
                Some((joined, inner)) => place_foreign(&placing, &joined, tab, inner)
                    .unwrap_or_else(|| {
                        view! {
                            <Frame />
                        }
                    }),
                None => {
                    provide_context(DockJoin {
                        host: tab,
                        registry: Rc::clone(&placing),
                        claimed: Rc::default(),
                    });
                    body.call(tab)
                }
            }
        }),
    };
    layout.inner().place.set(Some(body));
    let hosts = create_memo(move || {
        found.with(|found| {
            found
                .hosts
                .iter()
                .map(|(host, _, _)| *host)
                .collect::<Vec<_>>()
        })
    });
    if let Some(scope) = owner_scope() {
        link.place(DockBodies {
            scope,
            body,
            empty,
            hosts,
        });
    }
    let inner = layout.inner;
    on_cleanup(move || {
        link.withdraw();
        if let Some(inner) = inner.try_value() {
            *inner.host.borrow_mut() = None;
            inner.registry.host.borrow_mut().take();
            if inner.hosted.is_alive() {
                inner.hosted.set(None);
            }
        }
    });
    view! {
        <Frame />
    }
}
