use beui_view::types::icons::Glyph;
use std::hash::Hash;

use beui_dock_state::{DockAlign, DockBarPlacement, DockEdge, SIDEBAR_WIDTH, SplitSize, TabId};
use beui_macros::component;
use beui_view::reactive::*;
use beui_view::types::{Direction, Image, Rect, Vec2};

#[derive(Clone, Copy)]
pub(crate) struct DialogParts {
    pub(crate) title: Memo<String>,
    pub(crate) icon: Memo<Option<Glyph>>,
    pub(crate) size: Memo<Vec2>,
    pub(crate) modal: Memo<bool>,
    pub(crate) dismiss_on_outside_press: Memo<bool>,
    pub(crate) on_close: Option<ClickCallback>,
    pub(crate) content: RenderFn<()>,
}

impl DialogParts {
    pub(crate) fn alive(&self) -> bool {
        self.title.is_alive()
            && self.icon.is_alive()
            && self.size.is_alive()
            && self.modal.is_alive()
            && self.dismiss_on_outside_press.is_alive()
    }
}

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
    pub(crate) kind: NodeKind<K>,
}

pub(crate) enum NodeKind<K: 'static> {
    Split {
        key: String,
        direction: Direction,
        size: Memo<SplitSize>,
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
        parent: Memo<K>,
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
    pub(crate) kind: EntryKind<K>,
}

pub(crate) enum EntryKind<K: 'static> {
    Tab {
        id: K,
        title: Memo<String>,
        icon: Memo<Option<Glyph>>,
        picture: Memo<Option<Image>>,
        on_close: Option<ClickCallback>,
        back_closes: bool,
        see_through: bool,
        group: Memo<Option<String>>,
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
    #[prop(default = SplitSize::default())] size: Prop<SplitSize>,
    children: Children<DockNode<K>>,
) -> DockNode<K>
where
    K: DockKey,
{
    DockNode {
        kind: NodeKind::Split {
            key: id,
            direction,
            size: create_memo(move || size.get()),
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
    parent: Prop<K>,
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
            parent: create_memo(move || parent.get()),
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
    #[prop(default = true)] back_closes: bool,
    #[prop(default = false)] see_through: bool,
    #[prop(default = None)] group: Prop<Option<String>>,
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
            back_closes,
            see_through,
            group: create_memo(move || group.get()),
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
