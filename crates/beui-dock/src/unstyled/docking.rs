use beui_core::icons::Glyph;
mod switch;

use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::rc::Rc;

use beui_core::base::Direction;
use beui_core::color::Color;
use beui_core::geometry::{Rect, Vec2};
use beui_core::image::Image;
use beui_core::input::Modifiers;
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::*;
use serde::{Deserialize, Serialize};

use super::dialog::{DialogParts, DockDialogHandle, DockDialogPresentation, DockSheetHandle};
use super::state::{
    DockFullscreen, DockSpec, DockSpecEntry, DockSpecNode, DockSpecPane, DockSpecWindow,
};
use super::{
    Dock, DockApi, DockAwayHandle, DockBarHandle, DockChromeHandle, DockConfig, DockGripHandle,
    DockMode, DockPreviewHandle, DockSplitterHandle, DockStackHandle, DockState, DockSwitchHandle,
    DockSwitcherHandle, DockTabHandle, GroupId, LeafId, SIDEBAR_WIDTH, TabId,
};
use beui_components_unstyled::menu_popup::MenuStyle;

pub trait DockKey: Clone + Eq + Hash + 'static {}

impl<T: Clone + Eq + Hash + 'static> DockKey for T {}

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
    _scope: Scope,
    keys: RefCell<KeyMap<K>>,
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
        let inner = LayoutInner {
            _scope: owner.into_scope(),
            keys: RefCell::default(),
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

    pub fn restore(&self, snapshot: DockingSnapshot<K>) {
        let DockingSnapshot { mut state, keys } = snapshot;
        state.mark_seeded();
        let mut map = KeyMap::default();
        for (key, tab) in keys {
            map.next = map.next.max(tab.value() + 1);
            map.keys.insert(tab, key.clone());
            map.tabs.insert(key, tab);
        }
        *self.inner().keys.borrow_mut() = map;
        self.state.set(state);
        self.restores.update(|restores| *restores += 1);
    }

    pub fn reset(&self) {
        *self.inner().keys.borrow_mut() = KeyMap::default();
        self.state.set(DockState::default());
        self.restores.update(|restores| *restores += 1);
    }

    pub fn snapshot(&self) -> DockingSnapshot<K> {
        let state = self.state.get();
        let inner = self.inner();
        let keys = inner.keys.borrow();
        let mut held: Vec<(K, TabId)> = state
            .all_tabs()
            .into_iter()
            .filter_map(|tab| Some((keys.keys.get(&tab)?.clone(), tab)))
            .collect();
        held.sort_by_key(|(_, tab)| *tab);
        DockingSnapshot { state, keys: held }
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
        self.state
            .with(|state| tab.is_some_and(|tab| state.contains(tab)))
    }

    pub fn focused(&self) -> Option<K> {
        let tab = self.state.with(DockState::focused_tab)?;
        self.key_of(tab)
    }

    pub fn shown(&self) -> Option<K> {
        let tab = self.state.with(DockState::stacked_tab)?;
        self.key_of(tab)
    }

    pub fn show(&self, key: &K) {
        let Some(tab) = self.tab_of(key) else {
            return;
        };
        self.edit(|state| state.show(tab));
    }

    pub fn fullscreen(&self) -> Option<K> {
        let tab = self.state.with(DockState::fullscreen)?.tab;
        self.key_of(tab)
    }

    pub fn enter_fullscreen(&self, key: &K, area: Option<Rect>) {
        let Some(tab) = self.tab_of(key) else {
            return;
        };
        self.edit(|state| state.set_fullscreen(Some(DockFullscreen { tab, area })));
    }

    pub fn leave_fullscreen(&self) {
        self.edit(|state| state.set_fullscreen(None));
    }

    fn edit(&self, change: impl FnOnce(&mut DockState)) {
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
            .filter(|tab| !keys.keys.contains_key(tab));
        let tab = named.unwrap_or(TabId::new(keys.next));
        keys.next = keys.next.max(tab.value() + 1);
        keys.tabs.insert(key.clone(), tab);
        keys.keys.insert(tab, key.clone());
        tab
    }

    fn reconcile(&self, spec: &DockSpec, dialogs: &[(TabId, TabId)]) {
        let current = self.state.get_untracked();
        let mut next = current.clone();
        next.reconcile(spec);
        let mut kept: HashSet<TabId> = spec.tabs().into_iter().collect();
        kept.extend(next.all_tabs());
        kept.extend(
            dialogs
                .iter()
                .flat_map(|(dialog, parent)| [*dialog, *parent]),
        );
        {
            let inner = self.inner();
            let mut keys = inner.keys.borrow_mut();
            keys.keys.retain(|tab, _| kept.contains(tab));
            keys.tabs.retain(|_, tab| kept.contains(tab));
        }
        if next != current {
            self.state.set(next);
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
    dialogs: Vec<(TabId, TabId)>,
    parts: HashMap<TabId, DialogParts>,
}

impl<K: 'static> Default for Found<K> {
    fn default() -> Self {
        Self {
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

impl<K: 'static> Found<K> {
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
    create_effect(move || {
        layout.restores.get();
        let items = nodes.items();
        let mut held = Found::default();
        let focus = focus.get_untracked();
        let spec = layout.spec(&items, focus.as_ref(), &mut held);
        let dialogs = held.dialogs.clone();
        untrack(|| {
            found.set(Rc::new(held));
            layout.reconcile(&spec, &dialogs);
        });
    });
    let dialogs = create_memo(move || found.with(|found| found.dialogs.clone()));
    let dialog =
        Func::new(move |tab: TabId| found.with_untracked(|found| found.parts.get(&tab).cloned()));
    let title = Func::new(move |tab: TabId| {
        found
            .with(|found| found.tab(tab, |title, _, _, _| title.get()))
            .unwrap_or_default()
    });
    let icon = Func::new(move |tab: TabId| {
        found
            .with(|found| found.tab(tab, |_, icon, _, _| icon.get()))
            .unwrap_or_default()
    });
    let picture = Func::new(move |tab: TabId| found.with(|found| found.picture(tab)));
    let closable = Func::new(move |tab: TabId| {
        found
            .with(|found| found.tab(tab, |_, _, on_close, _| on_close.is_some()))
            .unwrap_or(false)
    });
    let on_close = move |tab: TabId| {
        let close = found
            .with_untracked(|found| found.tab(tab, |_, _, on_close, _| *on_close))
            .flatten();
        if let Some(close) = close {
            close.call();
        }
    };
    let content = RenderFn::new(move |tab: TabId| {
        let content = found.with_untracked(|found| found.tab(tab, |_, _, _, content| *content));
        let Some(content) = content else {
            return view! {
                <Frame />
            };
        };
        let see_through = found.with_untracked(|found| found.see_through(tab));
        let body = content.call(());
        match &frame {
            Some(frame) if !see_through => frame.call(body),
            _ => body,
        }
    });
    let empty = RenderFn::new(move |leaf: LeafId| {
        let key = layout
            .state
            .with_untracked(|state| state.pane_key(leaf).map(str::to_owned));
        let empty = key.and_then(|key| found.with_untracked(|found| found.empty(&key)));
        match empty {
            Some(empty) => empty.call(()),
            None => view! {
                <Frame />
            },
        }
    });
    let group_title = Func::new(move |group: GroupId| {
        let key = layout
            .state
            .with(|state| state.group_key(group).map(str::to_owned))?;
        let title = found.with(|found| found.group_title(&key))?;
        Some(title.get())
    });
    view! {
        <Dock
            config={DockConfig {
                api,
                state: Binding::from(layout.state),
                on_close: Callback::new(on_close),
                title,
                group_title,
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
