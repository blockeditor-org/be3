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
    pub(crate) on_close: Option<Callback<()>>,
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

/// A tab's key. `key()` names it as a stable string: saved layouts and a dock declared to a host
/// across apps use it, so it must not change between runs.
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
        hide_empty_bar: bool,
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
        on_close: Option<Callback<()>>,
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

/// Two children; `fraction` is the first one's share, followed until the user moves the splitter.
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

/// `id` is saved with the layout so a saved layout finds this pane after the code changes.
/// `bare` hides the tab bar: the pane holds one tab and takes no tab dropped onto it, only a split
/// at its edge (a joined dock's single pane uses it). A pane with an `empty` view is never closed:
/// emptied, it shows that view, under its bar's grip unless `hide_empty_bar` drops the bar while it
/// is empty. `locked` makes it an area that keeps its tabs (`DockState::home`).
#[component]
pub fn DockPane<K>(
    id: String,
    active: Option<K>,
    #[prop(default = false)] vertical: bool,
    #[prop(default = false)] icons_only: bool,
    #[prop(default = SIDEBAR_WIDTH)] sidebar_width: f32,
    #[prop(default = false)] locked: bool,
    #[prop(default = false)] bare: bool,
    #[prop(default = false)] hide_empty_bar: bool,
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
            hide_empty_bar,
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

/// A dialog hung from the tab (or dialog) `parent`, shown exactly while that tab is shown. It is
/// not part of the `DockState`, is never dragged and never saved; a key declared both as a dialog
/// and as a tab is only a dialog. `modal` blocks only the parent's panel: a press there flashes the
/// innermost modal dialog, unless `dismiss_on_outside_press` closes it, and the parent's tab label
/// shows a waiting dot.
#[component]
pub fn DockDialog<K>(
    id: K,
    parent: Prop<K>,
    title: Prop<String>,
    #[prop(default = None)] icon: Prop<Option<Glyph>>,
    size: Prop<Vec2>,
    #[prop(default = false)] modal: Prop<bool>,
    #[prop(default = false)] dismiss_on_outside_press: Prop<bool>,
    on_close: Option<Callback<()>>,
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

/// A tool bar its owner draws entirely, grip included. With a `parent` it shows only while that tab
/// is shown, wherever the tab goes; without one it belongs to the dock and wraps it. `placement` is
/// only where it starts: the user's placement is saved with the layout. A hidden bar keeps its body
/// built, and the dock offers no way to show it again, so an owner that lets it be hidden offers that.
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

/// A tab is closed when it stops being declared, even for a moment (a `Show` that flips, a
/// `ForEach` rebuilt from new keys), losing its place, history and panel. `on_close` only gives it a
/// close button and asks the caller to stop declaring it.
///
/// `back_closes=false` keeps back from closing a tab it opened (a program's window, whose close ends
/// the program). `see_through` shows what is drawn under the dock, so nothing fills behind it and it
/// never floats into a window. `group` names which tabs are one app's, for `next_of_group`.
/// `picture` is not carried across apps.
#[component]
pub fn DockTab<K>(
    id: K,
    title: Prop<String>,
    #[prop(default = None)] icon: Prop<Option<Glyph>>,
    #[prop(default = None)] picture: Prop<Option<Image>>,
    on_close: Option<Callback<()>>,
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
