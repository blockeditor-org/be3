use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::rc::Rc;

use beui_core::base::Direction;
use beui_core::geometry::Rect;
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::reactive::{
    Callback, ChildScope, ChildSegment, ChildValue, Children, ClickCallback, Frame, Func,
    IntoChild, IntoProp, IntoSegment, Memo, Prop, ReadSignal, RenderFn, Run, Scope, SlotChild,
    WriteSignal, clone, create_effect, create_memo, create_signal, untrack,
};
use serde::{Deserialize, Serialize};

use super::state::{DockSpec, DockSpecEntry, DockSpecNode, DockSpecPane, DockSpecWindow};
use super::{
    Dock, DockConfig, DockGripHandle, DockMode, DockPanelHandle, DockPreviewHandle,
    DockSplitterHandle, DockStackHandle, DockState, DockTabHandle, DockWindowHandle, GroupId,
    LeafId, SIDEBAR_WIDTH, SPLITTER_THICKNESS, TabId,
};
use crate::context_menu::MenuStyle;

pub trait DockKey: Clone + Eq + Hash + 'static {}

impl<T: Clone + Eq + Hash + 'static> DockKey for T {}

pub struct DockNode<K: 'static> {
    kind: NodeKind<K>,
    scope: ChildScope,
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
        sidebar: f32,
        empty: Option<RenderFn<()>>,
        entries: Run<DockEntry<K>>,
    },
    Window {
        key: String,
        rect: Rect,
        children: Run<DockNode<K>>,
    },
}

pub struct DockEntry<K: 'static> {
    kind: EntryKind<K>,
    scope: ChildScope,
}

enum EntryKind<K: 'static> {
    Tab {
        id: K,
        title: Memo<String>,
        icon: Memo<String>,
        on_close: Option<ClickCallback>,
        content: RenderFn<()>,
    },
    Group {
        key: String,
        title: Option<Memo<String>>,
        pinned: bool,
        children: Run<DockNode<K>>,
    },
}

macro_rules! dock_child {
    ($ty:ident) => {
        impl<K: 'static> ChildValue for $ty<K> {
            fn anchor(&self) -> Option<NodeId> {
                None
            }

            fn adopt_scope(&mut self, scope: Scope) {
                self.scope.adopt(scope);
            }
        }

        impl<K: 'static> IntoChild<$ty<K>> for $ty<K> {
            fn into_child(self) -> $ty<K> {
                self
            }
        }

        impl<K: 'static> IntoSegment<$ty<K>> for $ty<K> {
            fn into_segment(self) -> ChildSegment<$ty<K>> {
                ChildSegment::One(self)
            }
        }

        impl<K: 'static> From<$ty<K>> for Children<$ty<K>> {
            fn from(child: $ty<K>) -> Self {
                Self::from(ChildSegment::One(child))
            }
        }

        impl<K: 'static> SlotChild for $ty<K> {
            type Stored = Rc<$ty<K>>;

            fn store(self, _parent: Option<NodeId>) -> Self::Stored {
                Rc::new(self)
            }

            fn stored_node(_stored: &Self::Stored) -> Option<NodeId> {
                None
            }
        }
    };
}

dock_child!(DockNode);
dock_child!(DockEntry);

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
        scope: ChildScope::default(),
    }
}

#[component]
pub fn DockPane<K>(
    id: String,
    active: Option<K>,
    #[prop(default = false)] vertical: bool,
    #[prop(default = SIDEBAR_WIDTH)] sidebar_width: f32,
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
            sidebar: sidebar_width,
            empty,
            entries: children.into_run(),
        },
        scope: ChildScope::default(),
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
        scope: ChildScope::default(),
    }
}

#[component]
pub fn DockTab<K>(
    id: K,
    title: Prop<String>,
    #[prop(default = String::new())] icon: Prop<String>,
    on_close: Option<ClickCallback>,
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
            on_close,
            content,
        },
        scope: ChildScope::default(),
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
        scope: ChildScope::default(),
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

struct LayoutInner<K> {
    state: ReadSignal<DockState>,
    set_state: WriteSignal<DockState>,
    restores: ReadSignal<u64>,
    set_restores: WriteSignal<u64>,
    keys: RefCell<KeyMap<K>>,
}

pub struct DockingLayout<K> {
    inner: Rc<LayoutInner<K>>,
}

impl<K> Clone for DockingLayout<K> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl<K: DockKey> Default for DockingLayout<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: DockKey> DockingLayout<K> {
    pub fn new() -> Self {
        let (state, set_state) = create_signal(DockState::default());
        let (restores, set_restores) = create_signal(0);
        Self {
            inner: Rc::new(LayoutInner {
                state,
                set_state,
                restores,
                set_restores,
                keys: RefCell::default(),
            }),
        }
    }

    pub fn restore(&self, snapshot: DockingSnapshot<K>) {
        let DockingSnapshot { mut state, keys } = snapshot;
        state.mark_seeded();
        let mut map = KeyMap::default();
        for (key, tab) in keys {
            map.next = map.next.max(tab.value() + 1);
            map.keys.insert(tab, key.clone());
            map.tabs.insert(key, tab);
        }
        *self.inner.keys.borrow_mut() = map;
        self.inner.set_state.set(state);
        self.inner.set_restores.update(|restores| *restores += 1);
    }

    pub fn reset(&self) {
        *self.inner.keys.borrow_mut() = KeyMap::default();
        self.inner.set_state.set(DockState::default());
        self.inner.set_restores.update(|restores| *restores += 1);
    }

    pub fn snapshot(&self) -> DockingSnapshot<K> {
        let state = self.inner.state.get();
        let keys = self.inner.keys.borrow();
        let mut held: Vec<(K, TabId)> = state
            .all_tabs()
            .into_iter()
            .filter_map(|tab| Some((keys.keys.get(&tab)?.clone(), tab)))
            .collect();
        held.sort_by_key(|(_, tab)| *tab);
        DockingSnapshot { state, keys: held }
    }

    pub fn state(&self) -> ReadSignal<DockState> {
        self.inner.state.clone()
    }

    pub fn tab_of(&self, key: &K) -> Option<TabId> {
        self.inner.keys.borrow().tabs.get(key).copied()
    }

    pub fn key_of(&self, tab: TabId) -> Option<K> {
        self.inner.keys.borrow().keys.get(&tab).cloned()
    }

    pub fn contains(&self, key: &K) -> bool {
        let tab = self.tab_of(key);
        self.inner
            .state
            .with(|state| tab.is_some_and(|tab| state.contains(tab)))
    }

    pub fn focused(&self) -> Option<K> {
        let tab = self.inner.state.with(DockState::focused_tab)?;
        self.key_of(tab)
    }

    pub fn shown(&self) -> Option<K> {
        let tab = self.inner.state.with(DockState::stacked_tab)?;
        self.key_of(tab)
    }

    pub fn show(&self, key: &K) {
        let Some(tab) = self.tab_of(key) else {
            return;
        };
        self.edit(|state| state.show(tab));
    }

    fn edit(&self, change: impl FnOnce(&mut DockState)) {
        let current = self.inner.state.get_untracked();
        let mut next = current.clone();
        change(&mut next);
        if next != current {
            self.inner.set_state.set(next);
        }
    }

    fn mint(&self, key: &K) -> TabId {
        let mut keys = self.inner.keys.borrow_mut();
        if let Some(tab) = keys.tabs.get(key) {
            return *tab;
        }
        let named = key as &dyn Any;
        let named = named
            .downcast_ref::<TabId>()
            .copied()
            .or_else(|| named.downcast_ref::<u64>().copied().map(TabId::new))
            .filter(|tab| !keys.keys.contains_key(tab));
        let tab = named.unwrap_or(TabId::new(keys.next));
        keys.next = keys.next.max(tab.value() + 1);
        keys.tabs.insert(key.clone(), tab);
        keys.keys.insert(tab, key.clone());
        tab
    }

    fn reconcile(&self, spec: &DockSpec) {
        let current = self.inner.state.get_untracked();
        let mut next = current.clone();
        next.reconcile(spec);
        let mut kept: HashSet<TabId> = spec.tabs().into_iter().collect();
        kept.extend(next.all_tabs());
        {
            let mut keys = self.inner.keys.borrow_mut();
            keys.keys.retain(|tab, _| kept.contains(tab));
            keys.tabs.retain(|_, tab| kept.contains(tab));
        }
        if next != current {
            self.inner.set_state.set(next);
        }
    }

    fn spec(&self, nodes: &[Rc<DockNode<K>>], focus: Option<&K>, found: &mut Found<K>) -> DockSpec {
        let mut spec = DockSpec::default();
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
                _ if spec.main.is_none() => spec.main = self.node(node, found),
                _ => {}
            }
        }
        spec.focus = focus.map(|key| self.mint(key));
        spec
    }

    fn root(&self, children: &Run<DockNode<K>>, found: &mut Found<K>) -> Option<DockSpecNode> {
        children
            .items()
            .iter()
            .filter(|node| !matches!(node.kind, NodeKind::Window { .. }))
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
                sidebar,
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
                    sidebar: *sidebar,
                    keep: empty.is_some(),
                }))
            }
            NodeKind::Window { .. } => None,
        }
    }

    fn entry(&self, entry: &Rc<DockEntry<K>>, found: &mut Found<K>) -> Option<DockSpecEntry> {
        match &entry.kind {
            EntryKind::Tab { id, .. } => {
                let tab = self.mint(id);
                if found.tabs.insert(tab, Rc::clone(entry)).is_some() {
                    return None;
                }
                Some(DockSpecEntry::Tab(tab))
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

struct Found<K: 'static> {
    tabs: HashMap<TabId, Rc<DockEntry<K>>>,
    panes: HashMap<String, Rc<DockNode<K>>>,
    groups: HashMap<String, Rc<DockEntry<K>>>,
}

impl<K: 'static> Default for Found<K> {
    fn default() -> Self {
        Self {
            tabs: HashMap::new(),
            panes: HashMap::new(),
            groups: HashMap::new(),
        }
    }
}

impl<K: 'static> PartialEq for Found<K> {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

impl<K: 'static> Found<K> {
    fn tab<R>(
        &self,
        tab: TabId,
        read: impl FnOnce(&Memo<String>, &Memo<String>, &Option<ClickCallback>, &RenderFn<()>) -> R,
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

    fn empty(&self, key: &str) -> Option<RenderFn<()>> {
        match &self.panes.get(key)?.kind {
            NodeKind::Pane { empty, .. } => empty.clone(),
            _ => None,
        }
    }

    fn group_title(&self, key: &str) -> Option<Memo<String>> {
        match &self.groups.get(key)?.kind {
            EntryKind::Group { title, .. } => title.clone(),
            EntryKind::Tab { .. } => None,
        }
    }
}

#[component]
pub fn Docking<K>(
    layout: DockingLayout<K>,
    #[prop(default = DockMode::Tiled)] mode: Prop<DockMode>,
    #[prop(default = None)] home: Prop<Option<K>>,
    #[prop(default = None)] focus: Prop<Option<K>>,
    #[prop(default = MenuStyle::default())] menu: MenuStyle,
    #[prop(default = SPLITTER_THICKNESS)] splitter_thickness: f32,
    #[prop(default = 0.0)] group_inset: f32,
    #[prop(default = 0.0)] inset: Prop<f32>,
    tab: RenderFn<DockTabHandle>,
    frame: Option<RenderFn<NodeId>>,
    panel: Option<RenderFn<DockPanelHandle>>,
    splitter: Option<RenderFn<DockSplitterHandle>>,
    grip: Option<RenderFn<DockGripHandle>>,
    window: Option<RenderFn<DockWindowHandle>>,
    highlight: Option<RenderFn<()>>,
    preview: Option<RenderFn<DockPreviewHandle>>,
    stack: Option<RenderFn<DockStackHandle>>,
    children: Children<DockNode<K>>,
) -> NodeId
where
    K: DockKey,
{
    let nodes = children.into_run();
    let (found, set_found) = create_signal(Rc::new(Found::<K>::default()));
    create_effect(clone!(layout -> move || {
        layout.inner.restores.get();
        let items = nodes.items();
        let mut held = Found::default();
        let focus = focus.peek();
        let spec = layout.spec(&items, focus.as_ref(), &mut held);
        untrack(|| {
            set_found.set(Rc::new(held));
            layout.reconcile(&spec);
        });
    }));
    let titles = found.clone();
    let title = Func::new(move |tab: TabId| {
        titles
            .with(|found| found.tab(tab, |title, _, _, _| title.get()))
            .unwrap_or_default()
    });
    let icons = found.clone();
    let icon = Func::new(move |tab: TabId| {
        icons
            .with(|found| found.tab(tab, |_, icon, _, _| icon.get()))
            .unwrap_or_default()
    });
    let closers = found.clone();
    let closable = Func::new(move |tab: TabId| {
        closers
            .with(|found| found.tab(tab, |_, _, on_close, _| on_close.is_some()))
            .unwrap_or(false)
    });
    let closing = found.clone();
    let on_close = move |tab: TabId| {
        let close = closing
            .with_untracked(|found| found.tab(tab, |_, _, on_close, _| on_close.clone()))
            .flatten();
        if let Some(close) = close {
            close.call();
        }
    };
    let bodies = found.clone();
    let content = RenderFn::new(move |tab: TabId| {
        let content =
            bodies.with_untracked(|found| found.tab(tab, |_, _, _, content| content.clone()));
        let Some(content) = content else {
            return view! {
                <Frame />
            };
        };
        let body = content.call(());
        match &frame {
            Some(frame) => frame.call(body),
            None => body,
        }
    });
    let empties = found.clone();
    let emptied = layout.clone();
    let empty = RenderFn::new(move |leaf: LeafId| {
        let key = emptied
            .inner
            .state
            .with_untracked(|state| state.pane_key(leaf).map(str::to_owned));
        let empty = key.and_then(|key| empties.with_untracked(|found| found.empty(&key)));
        match empty {
            Some(empty) => empty.call(()),
            None => view! {
                <Frame />
            },
        }
    });
    let grouped = found.clone();
    let named = layout.clone();
    let group_title = Func::new(move |group: GroupId| {
        let key = named
            .inner
            .state
            .with(|state| state.group_key(group).map(str::to_owned))?;
        let title = grouped.with(|found| found.group_title(&key))?;
        Some(title.get())
    });
    let homed = layout.clone();
    let home = create_memo(move || {
        found.with(|_| ());
        home.get().and_then(|key| homed.tab_of(&key))
    });
    let state = layout.state();
    let changing = layout.clone();
    view! {
        <Dock
            config={DockConfig {
                state: state.into_prop(),
                on_change: Callback::new(move |next: DockState| {
                    changing.inner.set_state.set(next);
                }),
                on_close: Callback::new(on_close),
                title,
                group_title,
                icon,
                closable,
                menu,
                mode,
                home: home.into_prop(),
                splitter_thickness,
                group_inset,
                inset,
                tab,
                content,
                empty,
                panel,
                splitter,
                grip,
                window,
                highlight,
                preview,
                stack,
            }}
        />
    }
}
