use beui_view::types::icons::Glyph;
mod host;
mod switch;

use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
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
use beui_view::types::{Rect, Vec2};
use serde::{Deserialize, Serialize};

use super::dialog::{DialogParts, DockDialogHandle, DockDialogPresentation, DockSheetHandle};
use super::state::{
    DockFullscreen, DockMapping, DockSpec, DockSpecEntry, DockSpecNode, DockSpecPane,
    DockSpecWindow, joined_host, joined_key, joined_prefix,
};
use super::{
    Dock, DockApi, DockAwayHandle, DockBarHandle, DockChromeHandle, DockConfig, DockGripHandle,
    DockMode, DockPreviewHandle, DockSplitterHandle, DockStackHandle, DockState, DockSwitchHandle,
    DockSwitcherHandle, DockTabControl, DockTabHandle, DockTabMenu, GroupId, LeafId, SIDEBAR_WIDTH,
    TabId,
};
use beui_components_unstyled::DialogLayout;
use beui_components_unstyled::menu_popup::MenuStyle;
const MAX_JOINED_DEPTH: usize = 16;

pub use host::{
    DockBodies, DockDeclaration, DockDeclaredDialog, DockDeclaredTab, DockHostEvent, DockHostLink,
    DockHostSlot, DockHosted, DockJoin, DockRequest, use_dock_join,
};
use host::{Foreign, Joined, Registry};

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
    #[prop(default = DockDialogPresentation::Window)] presentation: DockDialogPresentation,
    #[prop(default = false)] dismiss_on_outside_press: Prop<bool>,
    #[prop(default = None)] anchor: Prop<Option<Rect>>,
    on_close: Option<ClickCallback>,
    #[prop(children)] content: RenderFn<()>,
) -> DockNode<K>
where
    K: DockKey,
{
    let card = presentation == DockDialogPresentation::Card;
    DockNode {
        kind: NodeKind::Dialog {
            id,
            parent,
            parts: DialogParts {
                presentation,
                title: create_memo(move || title.get()),
                icon: create_memo(move || icon.get()),
                size: create_memo(move || size.get()),
                modal: create_memo(move || card || modal.get()),
                dismiss_on_outside_press: create_memo(move || dismiss_on_outside_press.get()),
                anchor: create_memo(move || anchor.get().filter(|_| card)),
                on_close,
                content,
            },
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
    #[serde(default)]
    foreign: Vec<SavedForeign>,
    #[serde(default)]
    groups: Vec<(TabId, DockState)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SavedForeign {
    tab: TabId,
    host: TabId,
    path: String,
    title: String,
}

#[derive(Default)]
struct Saved {
    records: HashMap<TabId, SavedForeign>,
    parked: HashMap<TabId, TabId>,
    groups: HashMap<TabId, DockState>,
}

impl<K> DockingSnapshot<K> {
    pub fn new(state: DockState, keys: impl IntoIterator<Item = (K, TabId)>) -> Self {
        Self {
            state,
            keys: keys.into_iter().collect(),
            foreign: Vec::new(),
            groups: Vec::new(),
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
}

impl<K> Default for KeyMap<K> {
    fn default() -> Self {
        Self {
            tabs: HashMap::new(),
            keys: HashMap::new(),
            next: 1,
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
    registry: Rc<Registry>,
    host: RefCell<Option<Rc<dyn DockHostLink>>>,
    hosted: Signal<Option<DockHosted>>,
    declared: RefCell<Option<DockSpec>>,
    saved: RefCell<Saved>,
    dialogs: RefCell<HashMap<TabId, ForeignDialog>>,
    place: std::cell::Cell<Option<RenderFn<TabId, NodeId>>>,
}

pub struct DockingLayout<K: 'static> {
    state: Signal<DockState>,
    mode: Signal<DockMode>,
    restores: Signal<u64>,
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
        let registry = Rc::new(Registry {
            version: create_signal(0),
            joins: RefCell::default(),
            foreign: RefCell::default(),
            state,
            mode,
            above: hosted,
            host: RefCell::default(),
        });
        let inner = LayoutInner {
            _scope: owner.into_scope(),
            keys: RefCell::default(),
            registry,
            host: RefCell::default(),
            hosted,
            declared: RefCell::default(),
            saved: RefCell::default(),
            dialogs: RefCell::default(),
            place: std::cell::Cell::new(None),
        };
        let layout = move |inner| Self {
            state,
            mode,
            restores,
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
        let DockingSnapshot {
            mut state,
            keys,
            foreign,
            groups,
        } = snapshot;
        state.mark_seeded();
        let inner = self.inner();
        *inner.registry.foreign.borrow_mut() = Foreign::default();
        let mut map = self.fresh_keys();
        for (key, tab) in keys {
            map.next = map.next.max(tab.value() + 1);
            map.keys.insert(tab, key.clone());
            map.tabs.insert(key, tab);
        }
        for saved in &foreign {
            map.next = map.next.max(saved.tab.value() + 1);
        }
        *inner.keys.borrow_mut() = map;
        match self.host() {
            Some(host) => {
                *inner.saved.borrow_mut() = Saved::default();
                host.request(DockRequest::Restore(state));
            }
            None => {
                *inner.saved.borrow_mut() = Saved {
                    parked: foreign
                        .iter()
                        .filter(|saved| state.contains(saved.tab))
                        .map(|saved| (saved.tab, saved.host))
                        .collect(),
                    records: foreign
                        .into_iter()
                        .map(|saved| (saved.tab, saved))
                        .collect(),
                    groups: groups.into_iter().collect(),
                };
                self.state.set(state);
            }
        }
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
                *self.inner().saved.borrow_mut() = Saved::default();
                self.state.set(DockState::default());
            }
        }
        self.restores.update(|restores| *restores += 1);
    }

    pub fn snapshot(&self) -> DockingSnapshot<K> {
        let mut state = self.state.get();
        let inner = self.inner();
        let same = DockMapping {
            tab: &Some,
            key: &|key| Some(key.to_owned()),
            group: &joined_host,
        };
        let mut groups: Vec<(TabId, DockState)> = state
            .joined_groups()
            .into_iter()
            .map(|(group, host)| (host, state.subtree(group, &same)))
            .collect();
        let saved = inner.saved.borrow();
        for (host, subtree) in &saved.groups {
            if !groups.iter().any(|(held, _)| held == host) {
                groups.push((*host, subtree.clone()));
            }
        }
        groups.sort_by_key(|(host, _)| *host);
        while let Some((group, host)) = state.joined_groups().first().copied() {
            state.unhost_group(group, host);
        }
        let mut tabs: Vec<TabId> = state.all_tabs();
        tabs.extend(groups.iter().flat_map(|(_, subtree)| subtree.all_tabs()));
        tabs.sort();
        tabs.dedup();
        let foreign = tabs
            .iter()
            .filter_map(|tab| self.saved_foreign(*tab, &saved))
            .collect();
        let keys = inner.keys.borrow();
        let mut held: Vec<(K, TabId)> = state
            .all_tabs()
            .into_iter()
            .filter_map(|tab| Some((keys.keys.get(&tab)?.clone(), tab)))
            .collect();
        held.sort_by_key(|(_, tab)| *tab);
        DockingSnapshot {
            state,
            keys: held,
            foreign,
            groups,
        }
    }

    fn saved_foreign(&self, tab: TabId, saved: &Saved) -> Option<SavedForeign> {
        if let Some(record) = saved.records.get(&tab) {
            return Some(record.clone());
        }
        let registry = &self.inner().registry;
        let (host, inner) = registry.foreign.borrow().inner(tab)?;
        let joined = registry.joined(host)?;
        let declaration = joined.declaration.borrow();
        let declared = declaration.as_ref()?.tab(inner)?;
        Some(SavedForeign {
            tab,
            host,
            path: self.path(host, &declared.key, saved)?,
            title: declared.title.clone(),
        })
    }

    fn path(&self, host: TabId, key: &str, saved: &Saved) -> Option<String> {
        let mut path = key.to_owned();
        let mut host = host;
        for _ in 0..MAX_JOINED_DEPTH {
            if let Some(record) = saved.records.get(&host) {
                return Some(format!("{}/{path}", record.path));
            }
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

    fn parked_title(&self, tab: TabId) -> Option<String> {
        let saved = self.inner.try_value()?;
        let saved = saved.saved.borrow();
        saved
            .parked
            .contains_key(&tab)
            .then(|| saved.records.get(&tab).map(|record| record.title.clone()))
            .flatten()
    }

    fn close_parked(&self, tab: TabId) -> bool {
        let inner = self.inner();
        if inner.saved.borrow_mut().parked.remove(&tab).is_none() {
            return false;
        }
        inner.saved.borrow_mut().records.remove(&tab);
        self.edit(|state| {
            state.close(tab);
        });
        true
    }

    pub fn state(&self) -> ReadSignal<DockState> {
        self.state.read_only()
    }

    pub fn mode(&self) -> ReadSignal<DockMode> {
        self.mode.read_only()
    }

    pub fn tab_of(&self, key: &K) -> Option<TabId> {
        self.inner().keys.borrow().tabs.get(key).copied()
    }

    pub fn key_of(&self, tab: TabId) -> Option<K> {
        self.inner().keys.borrow().keys.get(&tab).cloned()
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
                !keys.keys.contains_key(tab)
                    && !inner.registry.foreign.borrow().contains(*tab)
                    && !inner.saved.borrow().records.contains_key(tab)
            });
        let tab = named.unwrap_or(TabId::new(keys.next));
        keys.next = keys.next.max(tab.value() + 1);
        keys.tabs.insert(key.clone(), tab);
        keys.keys.insert(tab, key.clone());
        tab
    }

    fn mint_foreign(&self, host: TabId, inner: TabId, key: Option<&str>) -> TabId {
        let layout = self.inner();
        if let Some(tab) = layout.registry.foreign.borrow().outer(host, inner) {
            return tab;
        }
        let reserved = key.and_then(|key| {
            let mut saved = layout.saved.borrow_mut();
            let path = self.path(host, key, &saved)?;
            let tab = saved
                .records
                .values()
                .find(|record| record.path == path)
                .map(|record| record.tab)?;
            saved.records.remove(&tab);
            Some(tab)
        });
        let mut keys = layout.keys.borrow_mut();
        let mut tab = TabId::new(keys.next);
        while keys.keys.contains_key(&tab) {
            tab = TabId::new(tab.value() + 1);
        }
        let tab = reserved
            .filter(|tab| !keys.keys.contains_key(tab))
            .unwrap_or(tab);
        keys.next = keys.next.max(tab.value() + 1);
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
        let parked = self.unpark(&mut next, spec, &live);
        next.reconcile_keeping(spec, &parked);
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
        self.retain_keys(&kept);
        registry
            .foreign
            .borrow_mut()
            .retain(|tab| kept.contains(&tab));
        if next != current {
            self.state.set(next);
        }
    }

    fn unpark(
        &self,
        next: &mut DockState,
        spec: &DockSpec,
        live: &HashSet<TabId>,
    ) -> HashSet<TabId> {
        let inner = self.inner();
        let mut saved = inner.saved.borrow_mut();
        if saved.parked.is_empty() && saved.groups.is_empty() {
            return HashSet::new();
        }
        let declared: HashSet<TabId> = spec.tabs().into_iter().collect();
        let held = |tab: TabId| declared.contains(&tab) || live.contains(&tab);
        let mut joined: Vec<(TabId, GroupId)> = next
            .joined_groups()
            .into_iter()
            .map(|(group, host)| (host, group))
            .filter(|(host, _)| live.contains(host))
            .collect();
        {
            let foreign = inner.registry.foreign.borrow();
            joined.sort_by_key(|(host, _)| {
                let mut depth = 0;
                let mut tab = *host;
                while let Some((outer, _)) = foreign.inner(tab)
                    && depth < MAX_JOINED_DEPTH
                {
                    depth += 1;
                    tab = outer;
                }
                depth
            });
        }
        for (host, group) in &joined {
            if let Some(subtree) = saved.groups.remove(host) {
                next.graft(
                    *group,
                    &subtree,
                    &DockMapping {
                        tab: &|tab| held(tab).then_some(tab),
                        key: &|key| Some(key.to_owned()),
                        group: &|_| None,
                    },
                );
            }
        }
        let parked: Vec<(TabId, TabId)> = saved
            .parked
            .iter()
            .map(|(tab, host)| (*tab, *host))
            .collect();
        for (tab, host) in parked {
            let group = joined
                .iter()
                .find(|(joined, _)| *joined == host)
                .map(|(_, group)| *group);
            if let Some(group) = group {
                saved.parked.remove(&tab);
                saved.records.remove(&tab);
                match declared.contains(&tab) {
                    true => next.rehome(tab, group),
                    false => {
                        next.close(tab);
                    }
                }
            } else if !held(host) && !saved.parked.contains_key(&host) {
                saved.parked.remove(&tab);
                saved.records.remove(&tab);
                next.close(tab);
            }
        }
        let Saved { parked, groups, .. } = &mut *saved;
        groups.retain(|host, _| held(*host) || parked.contains_key(host));
        parked.keys().copied().collect()
    }

    fn retain_keys(&self, kept: &HashSet<TabId>) {
        let inner = self.inner();
        let mut keys = inner.keys.borrow_mut();
        keys.keys.retain(|tab, _| kept.contains(tab));
        keys.tabs.retain(|_, tab| kept.contains(tab));
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
        self.retain_keys(&kept);
        self.inner()
            .registry
            .foreign
            .borrow_mut()
            .retain(|tab| kept.contains(&tab));
        *self.inner().declared.borrow_mut() = Some(spec.clone());
    }

    fn foreign_tabs(&self, found: &Found<K>) -> Vec<DockDeclaredTab> {
        let inner = self.inner();
        let saved = inner.saved.borrow();
        found
            .foreign
            .iter()
            .filter_map(|(tab, (joined, held))| {
                let declaration = joined.declaration.borrow();
                let declared = declaration.as_ref()?.tab(*held)?;
                let (host, _) = inner.registry.foreign.borrow().inner(*tab)?;
                Some(DockDeclaredTab {
                    tab: *tab,
                    key: self.path(host, &declared.key, &saved)?,
                    ..declared.clone()
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
                presentation: declared.presentation,
                title: create_memo(move || described.with(|described| described.title.clone())),
                icon: create_memo(move || described.with(|described| described.icon)),
                size: create_memo(move || described.with(|described| described.size)),
                modal: read(|described| {
                    described.modal || described.presentation == DockDialogPresentation::Card
                }),
                dismiss_on_outside_press: read(|described| described.dismiss_on_outside_press),
                anchor: create_memo(move || {
                    described.with(|described| {
                        described
                            .anchor
                            .filter(|_| described.presentation == DockDialogPresentation::Card)
                    })
                }),
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
                NodeKind::Dialog { .. } => {}
                _ if spec.main.is_none() => spec.main = self.node(node, found),
                _ => {}
            }
        }
        spec.windows.append(&mut found.windows);
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
            .filter(|node| !matches!(node.kind, NodeKind::Window { .. } | NodeKind::Dialog { .. }))
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
                    keep: empty.is_some(),
                    locked: *locked,
                }))
            }
            NodeKind::Window { .. } | NodeKind::Dialog { .. } => None,
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
        let key_of = |inner: TabId| {
            joined
                .declaration
                .borrow()
                .as_ref()
                .and_then(|declaration| declaration.tab(inner))
                .map(|declared| declared.key.clone())
        };
        let focus = spec
            .focus
            .map(|inner| self.mint_foreign(host, inner, key_of(inner).as_deref()));
        let dialogs = joined
            .declaration
            .borrow()
            .as_ref()
            .map(|declaration| declaration.dialogs.clone())
            .unwrap_or_default();
        for dialog in &dialogs {
            let tab = self.mint_foreign(host, dialog.tab, None);
            let parent = self.mint_foreign(host, dialog.parent, key_of(dialog.parent).as_deref());
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
                            let key = joined
                                .declaration
                                .borrow()
                                .as_ref()
                                .and_then(|declaration| declaration.tab(*inner))
                                .map(|declared| declared.key.clone());
                            let tab = self.mint_foreign(host, *inner, key.as_deref());
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
                    active: pane.active.map(|inner| {
                        let key = joined
                            .declaration
                            .borrow()
                            .as_ref()
                            .and_then(|declaration| declaration.tab(inner))
                            .map(|declared| declared.key.clone());
                        self.mint_foreign(host, inner, key.as_deref())
                    }),
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
}

impl<K: 'static> Default for Found<K> {
    fn default() -> Self {
        Self {
            foreign: HashMap::new(),
            hosts: Vec::new(),
            windows: Vec::new(),
            tabs: HashMap::new(),
            panes: HashMap::new(),
            groups: HashMap::new(),
            dialogs: Vec::new(),
            parts: HashMap::new(),
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
                    presentation: parts.presentation,
                    dismiss_on_outside_press: parts.dismiss_on_outside_press.get(),
                    anchor: parts.anchor.get(),
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
            EntryKind::Tab { picture, .. } => picture.get(),
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
    grip: Option<RenderFn<DockGripHandle>>,
    highlight: Option<RenderFn<()>>,
    preview: Option<RenderFn<DockPreviewHandle>>,
    stack: Option<RenderFn<DockStackHandle>>,
    switcher: Option<RenderFn<DockSwitcherHandle>>,
    switch: Option<RenderFn<DockSwitchHandle>>,
    dialog_chrome: Option<RenderFn<DockDialogHandle>>,
    dialog_sheet: Option<RenderFn<DockSheetHandle>>,
    dialog_scrim: Prop<Color>,
    #[prop(default = 0)] dialog_margin: u16,
    #[prop(default = 0)] dialog_gap: u16,
    #[prop(default = 0.0)] dialog_beak: f32,
    #[prop(default = Vec2::ZERO)] dialog_outset: Vec2,
    children: Children<DockNode<K>>,
) -> NodeId
where
    K: DockKey,
{
    let nodes = children.into_run();
    let found = create_signal(Rc::new(Found::<K>::default()));
    let on_close = move |tab: TabId| {
        if layout.close_parked(tab) {
            return;
        }
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
    let link = match use_dock_join().and_then(|join| join.claim()) {
        Some(link) => Some((link, false)),
        None => use_context::<DockHostSlot>()
            .and_then(|slot| slot.claim())
            .map(|link| (link, true)),
    };
    if let Some((link, nests)) = link {
        let on_close = Callback::new(on_close);
        return view! {
            <JoinedDock layout link nests nodes found body focus on_close />
        };
    }
    let registry = Rc::clone(&layout.inner().registry);
    let version = registry.version;
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
            layout.reconcile(&spec, &dialogs, &hosts);
        });
    });
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
        if let Some(title) = layout.parked_title(tab) {
            return title;
        }
        found
            .with(|found| found.tab(tab, |title, _, _, _| title.get()))
            .unwrap_or_default()
    });
    let icon = Func::new(move |tab: TabId| {
        if let Some(declared) = declared(tab) {
            return declared.icon;
        }
        found
            .with(|found| found.tab(tab, |_, icon, _, _| icon.get()))
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
        if layout.parked_title(tab).is_some() {
            return true;
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
            return body.call(tab);
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
            found.foreign.contains_key(&tab) || found.tabs.contains_key(&tab)
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
        let empty = key.and_then(|key| found.with_untracked(|found| found.empty(&key)));
        match empty {
            Some(empty) => empty.call(()),
            None => view! {
                <Frame />
            },
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
                grip,
                highlight,
                preview,
                stack,
                switcher,
                switch,
                dialogs,
                dialog,
                dialog_chrome,
                dialog_sheet,
                dialog_scrim: create_memo(move || dialog_scrim.get()),
                dialog_margin,
                dialog_gap,
                dialog_beak,
                dialog_outset,
            }}
        />
    }
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
    let menu = use_context::<DockTabMenu>();
    let layout = use_context::<DialogLayout>();
    let registry = Rc::clone(registry);
    let placed = scope.run(|| {
        if let Some(control) = control {
            provide_context(control);
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
        let (mut tabs, mut dialogs) =
            found.with(|found| (found.declared_tabs(), found.declared_dialogs()));
        tabs.extend(found.with_untracked(|found| layout.foreign_tabs(found)));
        let hosts: Vec<TabId> =
            found.with(|found| found.hosts.iter().map(|(host, _, _)| *host).collect());
        tabs.retain(|declared| !hosts.contains(&declared.tab));
        tabs.sort_by_key(|declared| declared.tab);
        dialogs.sort_by_key(|declared| declared.tab);
        untrack(|| {
            declaring.declare(DockDeclaration {
                spec,
                tabs,
                dialogs,
            })
        });
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
