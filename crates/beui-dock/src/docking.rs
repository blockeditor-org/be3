use be_lib_stable_hash::{HashMap, HashSet};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui_dock_state::{
    DockBarPlacement, DockMode, DockSpec, DockSpecBar, DockSpecEntry, DockSpecNode, DockSpecPane,
    DockSpecWindow, DockState, TabId, joined_host, joined_key, joined_prefix,
};
use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Rect;

use crate::controls::{DockBarControl, DockTabControl, DockTabMenu};
use crate::declare::{DialogParts, DockEntry, DockKey, DockNode, EntryKind, NodeKind};
use crate::link::{
    DockBodies, DockDeclaration, DockDeclaredBar, DockDeclaredDialog, DockDeclaredGroup,
    DockDeclaredHost, DockDeclaredTab, DockHostEvent, DockHostLink, DockHostSlot, DockHosted,
    DockJoin, DockKeeper, DockKept, DockRequest, Joined, Registry, use_dock_join,
};
use beui_components_unstyled::DialogLayout;

const MAX_JOINED_DEPTH: usize = 16;

#[derive(Clone, Debug, PartialEq)]
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

struct LayoutInner<K: 'static> {
    _scope: Scope,
    keys: RefCell<KeyMap<K>>,
    keyed: Signal<u64>,
    registry: Rc<Registry>,
    host: RefCell<Option<Rc<dyn DockHostLink>>>,
    hosted: Signal<Option<DockHosted>>,
    declared: RefCell<Option<DockSpec>>,
    dialogs: RefCell<HashMap<TabId, ForeignDialog>>,
    place: Cell<Option<RenderFn<TabId, NodeId>>>,
}

/// The caller's handle on a dock: where everything is, keyed by the caller's `DockKey`. `Copy`, and
/// owned by the scope that made it; `OwnedDockingLayout` makes one outside a scope. In a dock that
/// joined another, every call goes through the outer dock.
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
            above: hosted,
            host: RefCell::default(),
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
            place: Cell::new(None),
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

    fn ask(&self, request: DockRequest) {
        if let Some(host) = self.host() {
            host.request(request);
        }
    }

    fn hosted<R>(&self, read: impl FnOnce(&DockHosted) -> R) -> Option<R> {
        self.inner().hosted.with(|hosted| hosted.as_ref().map(read))
    }

    fn fresh_keys(&self) -> KeyMap<K> {
        let mut map = KeyMap::default();
        map.next = self.inner().registry.foreign.borrow().newest() + 1;
        map
    }

    /// Call before the effects of the write that declares the snapshot's tabs run (writes made
    /// together, with no `flush()` between, qualify): a tab not declared when the dock reconciles is
    /// closed. For a joined dock, a restore made before it joins is grafted in once its group stands.
    pub fn restore(&self, snapshot: DockingSnapshot<K>) {
        let DockingSnapshot { state, keys } = snapshot;
        let inner = self.inner();
        let mut map = self.fresh_keys();
        for (key, tab) in keys {
            map.next = map.next.max(tab.value() + 1);
            map.keys.insert(tab, key.clone());
            map.tabs.insert(key, tab);
        }
        *inner.keys.borrow_mut() = map;
        self.keep_state(state);
    }

    fn keep_state(&self, mut state: DockState) {
        state.mark_seeded();
        self.inner().registry.keep_joined();
        match self.host() {
            Some(host) => host.request(DockRequest::Restore(state)),
            None => self.state.set(state),
        }
        self.restores.update(|restores| *restores += 1);
    }

    pub fn reset(&self) {
        self.inner().registry.keep_joined();
        match self.host() {
            Some(host) => host.request(DockRequest::Restore(DockState::default())),
            None => self.state.set(DockState::default()),
        }
        self.restores.update(|restores| *restores += 1);
    }

    /// Has no serialized form: save be-sdk-beui's `SavedDockLayout::of(snapshot)`. Fullscreen, snaps,
    /// hidden windows, dialogs and a switch in progress are not in it. The outer dock's snapshot keeps
    /// only a joined dock's hosting tab; the joined dock's owner saves this one.
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
            None => self.state.with(DockState::focused_or_host)?,
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
        if let Some(tab) = self.tab_of(key) {
            self.ask(DockRequest::Show(tab));
        }
    }

    pub fn bar_placement(&self, key: &K) -> Option<DockBarPlacement> {
        let bar = self.tab_of(key)?;
        self.state.with(|state| state.bar_placement(bar))
    }

    pub fn place_bar(&self, key: &K, placement: DockBarPlacement) {
        if let Some(bar) = self.tab_of(key) {
            self.ask(DockRequest::PlaceBar(bar, Some(placement)));
        }
    }

    pub fn is_bar_hidden(&self, key: &K) -> bool {
        let Some(bar) = self.tab_of(key) else {
            return false;
        };
        self.state.with(|state| state.is_bar_hidden(bar))
    }

    pub fn hide_bar(&self, key: &K) {
        self.set_bar_hidden(key, true);
    }

    pub fn show_bar(&self, key: &K) {
        self.set_bar_hidden(key, false);
    }

    fn set_bar_hidden(&self, key: &K, hidden: bool) {
        if let Some(bar) = self.tab_of(key) {
            self.ask(DockRequest::HideBar(bar, hidden));
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
        if let Some(tab) = self.tab_of(key) {
            self.ask(DockRequest::Fullscreen(Some((tab, area))));
        }
    }

    pub fn leave_fullscreen(&self) {
        self.ask(DockRequest::Fullscreen(None));
    }

    fn declaration(&self, spec: DockSpec, found: Signal<Rc<Found<K>>>) -> DockDeclaration {
        let (mut tabs, own_dialogs, own_groups) = found.with(|found| {
            (
                found.declared_tabs(),
                found.declared_dialogs(),
                found.declared_groups(),
            )
        });
        let (foreign_tabs, foreign_groups) =
            found.with_untracked(|found| (self.foreign_tabs(found), self.foreign_groups(found)));
        tabs.extend(foreign_tabs);
        let hosted: Vec<(TabId, DockSpecNode, Option<TabId>)> =
            found.with(|found| found.hosts.clone());
        let mut hosts = Vec::new();
        for (host, root, focus) in hosted {
            if let Some(tab) = tabs.iter().find(|declared| declared.tab == host).cloned() {
                hosts.push(DockDeclaredHost { tab, root, focus });
            }
        }
        tabs.retain(|declared| hosts.iter().all(|host| host.tab.tab != declared.tab));
        tabs.sort_by_key(|declared| declared.tab);
        let mut dialogs: Vec<DockDeclaredDialog> = own_dialogs
            .into_iter()
            .filter_map(|(dialog, own)| {
                let key = match own {
                    true => self.key_of(dialog.tab)?.key(),
                    false => found.with_untracked(|found| self.foreign_key(found, dialog.tab))?,
                };
                Some(DockDeclaredDialog { key, ..dialog })
            })
            .collect();
        dialogs.sort_by_key(|declared| declared.tab);
        let mut groups = own_groups;
        groups.extend(foreign_groups);
        groups.sort_by(|left, right| left.key.cmp(&right.key));
        let bars = found.with_untracked(|found| self.declared_bars(&spec, found));
        DockDeclaration {
            spec,
            tabs,
            dialogs,
            bars,
            groups,
            hosts,
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
        kept.extend(self.state.with_untracked(DockState::all_tabs));
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

    fn foreign_key(&self, found: &Found<K>, tab: TabId) -> Option<String> {
        let (joined, held) = found.foreign.get(&tab)?;
        let declaration = joined.declaration.borrow();
        let declaration = declaration.as_ref()?;
        let key = match declaration.tab(*held) {
            Some(declared) => declared.key.clone(),
            None => declaration.dialog(*held)?.key.clone(),
        };
        let (host, _) = self.inner().registry.foreign.borrow().inner(tab)?;
        self.path(host, &key)
    }

    fn foreign_tabs(&self, found: &Found<K>) -> Vec<DockDeclaredTab> {
        found
            .foreign
            .iter()
            .filter_map(|(tab, (joined, held))| {
                let declaration = joined.declaration.borrow();
                let declared = declaration.as_ref()?.tab(*held)?;
                Some(DockDeclaredTab {
                    tab: *tab,
                    key: self.foreign_key(found, *tab)?,
                    ..declared.clone()
                })
            })
            .collect()
    }

    fn foreign_groups(&self, found: &Found<K>) -> Vec<DockDeclaredGroup> {
        let registry = &self.inner().registry;
        found
            .hosts
            .iter()
            .filter_map(|(host, _, _)| {
                let joined = registry.joined(*host)?;
                let declaration = joined.declaration.borrow();
                let prefix = joined_prefix(*host);
                Some(
                    declaration
                        .as_ref()?
                        .groups
                        .iter()
                        .map(|group| DockDeclaredGroup {
                            key: format!("{prefix}{}", group.key),
                            title: group.title.clone(),
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .flatten()
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
                let owner = match bar.owner {
                    Some(owner) => Some(spec.owner_tab(owner)?),
                    None => None,
                };
                Some(DockDeclaredBar {
                    tab: bar.bar,
                    key,
                    kind,
                    owner,
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
        let grafted = self
            .state
            .with_untracked(|state| inner.registry.graft_pending(state));
        if let Some(grafted) = grafted {
            self.ask(DockRequest::Restore(grafted));
        }
    }

    fn spec(&self, nodes: &[Rc<DockNode<K>>], focus: Option<&K>, found: &mut Found<K>) -> DockSpec {
        let mut spec = DockSpec::default();
        for node in nodes {
            if let NodeKind::Dialog { id, parent, parts } = &node.kind {
                let dialog = self.mint(id);
                if found.dialogs.iter().any(|(held, _)| *held == dialog) {
                    continue;
                }
                let parent = parent.with(|parent| self.mint(parent));
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
                    hidden: false,
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
            .chain(found.see_through.iter().copied())
            .collect();
        spec.see_through.sort();
        spec.see_through.dedup();
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
                size,
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
                    size: size.get(),
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
                let entries: Vec<DockSpecEntry> = entries
                    .items()
                    .iter()
                    .filter_map(|entry| self.entry(entry, found))
                    .collect();
                let active = active
                    .as_ref()
                    .map(|key| DockSpecEntry::Tab(self.mint(key)))
                    .and_then(|active| entries.iter().position(|entry| *entry == active))
                    .unwrap_or(0);
                Some(DockSpecNode::Pane(DockSpecPane {
                    key: key.clone(),
                    entries,
                    active,
                    vertical: *vertical,
                    icons_only: *icons_only,
                    sidebar: *sidebar,
                    keep: empty.is_some() || *bare,
                    area: locked.then(|| key.clone()),
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

    fn splice(&self, host: TabId, found: &mut Found<K>) -> Option<DockSpecEntry> {
        let registry = Rc::clone(&self.inner().registry);
        let joined = registry.joined(host)?;
        let spec = joined.declaration.borrow().as_ref()?.spec.clone();
        let root = spec.main.as_ref()?;
        let root = self.foreign_node(host, &joined, root, found);
        let focus = spec.focus.map(|inner| self.mint_foreign(host, inner));
        for inner in &spec.see_through {
            let tab = self.mint_foreign(host, *inner);
            found.see_through.push(tab);
        }
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
                hidden: false,
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
                size,
                first,
                second,
            } => DockSpecNode::Split {
                key: format!("{prefix}{key}"),
                direction: *direction,
                size: *size,
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
                    area: pane.area.as_ref().map(|area| format!("{prefix}{area}")),
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
    see_through: Vec<TabId>,
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
            see_through: Vec::new(),
        }
    }
}

impl<K: 'static> PartialEq for Found<K> {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

impl<K: DockKey> Found<K> {
    fn live_parts(&self, tab: TabId) -> Option<DialogParts> {
        self.parts.get(&tab).copied().filter(DialogParts::alive)
    }

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
                    back_closes,
                    group,
                    ..
                } if title.is_alive() && icon.is_alive() && picture.is_alive() => {
                    Some(DockDeclaredTab {
                        tab: *tab,
                        key: id.key(),
                        title: title.get(),
                        icon: icon.get(),
                        picture: picture.get(),
                        closable: on_close.is_some(),
                        back_closes: *back_closes,
                        group: group.is_alive().then(|| group.get()).flatten(),
                    })
                }
                EntryKind::Tab { .. } | EntryKind::Group { .. } => None,
            })
            .collect()
    }

    fn declared_groups(&self) -> Vec<DockDeclaredGroup> {
        self.groups
            .iter()
            .filter_map(|(key, entry)| match &entry.kind {
                EntryKind::Group {
                    title: Some(title), ..
                } if title.is_alive() => Some(DockDeclaredGroup {
                    key: key.clone(),
                    title: title.get(),
                }),
                _ => None,
            })
            .collect()
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

    fn declared_dialogs(&self) -> Vec<(DockDeclaredDialog, bool)> {
        self.dialogs
            .iter()
            .filter_map(|(tab, parent)| {
                let parts = self.live_parts(*tab)?;
                Some((
                    DockDeclaredDialog {
                        tab: *tab,
                        key: String::new(),
                        parent: *parent,
                        title: parts.title.get(),
                        icon: parts.icon.get(),
                        size: parts.size.get(),
                        modal: parts.modal.get(),
                        dismiss_on_outside_press: parts.dismiss_on_outside_press.get(),
                        closable: parts.on_close.is_some(),
                    },
                    !self.foreign.contains_key(tab),
                ))
            })
            .collect()
    }

    fn content(&self, tab: TabId) -> Option<RenderFn<()>> {
        match &self.tabs.get(&tab)?.kind {
            EntryKind::Tab { content, .. } => Some(*content),
            EntryKind::Group { .. } => None,
        }
    }

    fn on_close(&self, tab: TabId) -> Option<ClickCallback> {
        match &self.tabs.get(&tab)?.kind {
            EntryKind::Tab { on_close, .. } => *on_close,
            EntryKind::Group { .. } => None,
        }
    }

    fn empty(&self, key: &str) -> Option<RenderFn<()>> {
        match &self.panes.get(key)?.kind {
            NodeKind::Pane { empty, .. } => *empty,
            _ => None,
        }
    }
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
        provide_context(DockJoin::new(tab, registry));
        bodies.map(|bodies| bodies.body.call(inner))
    });
    on_cleanup(move || drop(scope));
    placed
}

#[component]
pub fn Docking<K>(
    layout: DockingLayout<K>,
    #[prop(default = None)] focus: Prop<Option<K>>,
    children: Children<DockNode<K>>,
) -> NodeId
where
    K: DockKey,
{
    let nodes = children.into_run();
    let found = create_signal(Rc::new(Found::<K>::default()));
    let on_close = Callback::new(move |tab: TabId| {
        let close = found.with_untracked(|found| match found.foreign.get(&tab) {
            Some((joined, inner)) => Err((Rc::clone(joined), *inner)),
            None => Ok(found
                .on_close(tab)
                .or_else(|| found.live_parts(tab).and_then(|parts| parts.on_close))),
        });
        match close {
            Ok(Some(close)) => close.call(),
            Ok(None) => {}
            Err((joined, inner)) => joined.close(inner),
        }
    });
    let body = RenderFn::new(move |tab: TabId| {
        if let Some(bar) = found.with_untracked(|found| found.bars.get(&tab).map(|(_, bar)| *bar)) {
            return bar.call(());
        }
        if let Some(content) = found.with_untracked(|found| found.content(tab)) {
            return content.call(());
        }
        let dialog = found.with_untracked(|found| found.live_parts(tab).map(|parts| parts.content));
        match dialog {
            Some(dialog) => dialog.call(()),
            None => view! {
                <Frame />
            },
        }
    });
    let slot = use_context::<DockHostSlot>();
    let joined = use_dock_join().and_then(|join| join.claim());
    let (link, nests, local) = match joined {
        Some(link) => (link, false, false),
        None => {
            let slot = slot.expect("a Docking is built under a DockHost or inside a dock's tab");
            let link = slot
                .claim()
                .expect("a DockHost hosts one Docking; give each Docking a DockHost of its own");
            (link, true, slot.is_local())
        }
    };
    let keeper = match local {
        true => use_context::<DockKeeper>().and_then(|keeper| keeper.claim()),
        false => None,
    };
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
        if let Some(empty) = found.with_untracked(|found| found.empty(&key)) {
            return Some(empty.call(()));
        }
        let declared = {
            let key = key.clone();
            create_memo(move || found.with(|found| found.empty(&key).is_some()))
        };
        Some(view! {
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
        })
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
                    provide_context(DockJoin::new(tab, Rc::clone(&placing)));
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
    if let Some(keep) = keeper {
        keep(DockKept {
            layout: create_memo(move || layout.snapshot().state),
            keys: create_memo(move || found.with(|found| found.own_keys())),
            restore: Callback::new(move |state: DockState| layout.keep_state(state)),
        });
    }
    let inner = layout.inner;
    let withdrawing = Rc::clone(&link);
    on_cleanup(move || {
        withdrawing.withdraw();
        if let Some(inner) = inner.try_value() {
            *inner.host.borrow_mut() = None;
            inner.registry.host.borrow_mut().take();
            if inner.hosted.is_alive() {
                inner.hosted.set(None);
            }
        }
    });
    link.view().unwrap_or_else(|| {
        view! {
            <Frame />
        }
    })
}
