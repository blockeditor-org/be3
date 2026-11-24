use be_lib_stable_hash::HashSet;
use beui_view::types::icons::Glyph;
use std::any::Any;
use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;

use beui_dock_state::{DockAlign, DockBarPlacement, DockEdge, SIDEBAR_WIDTH, SplitSize, TabId};
use beui_view::reactive::macro_support::OptionalTakes;
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
    pub(crate) content: BuildFn<NodeId>,
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

/// A tab's id as the declarations hold it: a caller's `DockKey`, or one the dock derived for a tab
/// declared without one. Any `DockKey` passes as one, so the declarations need no key type and a
/// dock whose tabs are all derived names none.
#[derive(Clone)]
pub struct DockId {
    key: String,
    value: Option<Rc<dyn Any>>,
}

impl DockId {
    pub fn of<K: DockKey>(key: &K) -> Self {
        Self {
            key: key.key(),
            value: Some(Rc::new(key.clone())),
        }
    }

    pub(crate) fn derived(key: String) -> Self {
        Self { key, value: None }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub(crate) fn is_derived(&self) -> bool {
        self.value.is_none()
    }

    pub(crate) fn value(&self) -> Option<&dyn Any> {
        self.value.as_deref()
    }

    /// The caller's key, for a tab that has one. A dock read through a `DockingLayout<K>` keys
    /// every tab it gives an id with a `K`.
    pub(crate) fn as_key<K: DockKey>(&self) -> Option<K> {
        let key = self.value()?.downcast_ref::<K>().cloned();
        debug_assert!(
            key.is_some(),
            "the tab {:?} is keyed by another type than its layout's ({})",
            self.key,
            std::any::type_name::<K>()
        );
        key
    }
}

impl PartialEq for DockId {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.is_derived() == other.is_derived()
    }
}

impl Eq for DockId {}

impl Hash for DockId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key.hash(state);
        self.is_derived().hash(state);
    }
}

impl std::fmt::Debug for DockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DockId({:?})", self.key)
    }
}

pub struct ByDockKey;

impl<K: DockKey> OptionalTakes<K, ByDockKey> for DockId {
    fn take_some(value: K) -> Self {
        DockId::of(&value)
    }
}

/// A key the caller gave, or the one derived the first time the dock read it: kept, so a node
/// keeps its key while it is declared whatever its siblings do.
pub(crate) struct Keyed<T> {
    given: Option<T>,
    derived: RefCell<Option<T>>,
}

impl<T: Clone + Eq + Hash> Keyed<T> {
    fn new(given: Option<T>) -> Self {
        Self {
            given,
            derived: RefCell::new(None),
        }
    }

    /// The key, once the dock has read it.
    pub(crate) fn keyed(&self) -> T {
        self.known().expect("the dock keys a node when it reads it")
    }

    /// The key given or kept, before this read.
    pub(crate) fn known(&self) -> Option<T> {
        self.given.clone().or_else(|| self.derived.borrow().clone())
    }

    /// The key given or kept, else `derive`'s, stepped past every key `held` already (with
    /// `step(base, n)`), and held from then on. `held` starts with every key given or kept, so
    /// a node declared before one that already has a key never takes it.
    pub(crate) fn get(
        &self,
        held: &mut HashSet<T>,
        derive: impl FnOnce() -> T,
        step: impl Fn(&T, usize) -> T,
    ) -> T {
        if let Some(known) = self.known() {
            held.insert(known.clone());
            return known;
        }
        let base = derive();
        let mut key = base.clone();
        let mut next = 1;
        while held.contains(&key) {
            key = step(&base, next);
            next += 1;
        }
        held.insert(key.clone());
        *self.derived.borrow_mut() = Some(key.clone());
        key
    }
}

pub struct DockNode {
    pub(crate) kind: NodeKind,
}

pub(crate) enum NodeKind {
    Split {
        key: Keyed<String>,
        direction: Direction,
        size: Option<Memo<SplitSize>>,
        children: Run<DockNode>,
    },
    Pane {
        key: Keyed<String>,
        active: Option<DockId>,
        vertical: bool,
        icons_only: bool,
        sidebar: f32,
        locked: bool,
        bare: bool,
        stage: bool,
        chrome: bool,
        hide_empty_bar: bool,
        empty: Option<BuildFn<NodeId>>,
        entries: Run<DockEntry>,
    },
    Window {
        key: String,
        rect: Rect,
        hidden: bool,
        children: Run<DockNode>,
    },
    Dialog {
        id: DockId,
        parent: Memo<DockId>,
        parts: DialogParts,
    },
    Bar {
        id: DockId,
        parent: Option<DockId>,
        placement: DockBarPlacement,
        content: BuildFn<NodeId>,
    },
}

pub struct DockEntry {
    pub(crate) kind: EntryKind,
}

pub(crate) enum EntryKind {
    Tab {
        id: Keyed<DockId>,
        title: Memo<String>,
        icon: Memo<Option<Glyph>>,
        picture: Memo<Option<Image>>,
        on_close: Option<Callback<()>>,
        back_closes: bool,
        see_through: bool,
        bare: bool,
        group: Memo<Option<String>>,
        content: BuildFn<NodeId>,
    },
    Group {
        key: String,
        title: Option<Memo<String>>,
        pinned: bool,
        children: Run<DockNode>,
    },
}

beui_view::value_child_type!(DockNode);
beui_view::value_child_type!(DockEntry);

/// Two children; `size` is the first one's share, followed until the user moves the splitter. Left
/// out, a split with the stage on one side gives the other side `SIDE_PANEL_WIDTH`, and any other
/// split halves. `id` is saved with the layout; left out, it is the split's place under its parent.
#[component]
pub fn DockSplit(
    id: Option<String>,
    #[prop(default = Direction::Horizontal)] direction: Direction,
    size: Option<Prop<SplitSize>>,
    children: Children<DockNode>,
) -> DockNode {
    DockNode {
        kind: NodeKind::Split {
            key: Keyed::new(id),
            direction,
            size: size.map(|size| create_memo(move || size.get())),
            children: children.into_run(),
        },
    }
}

/// `id` is saved with the layout so a saved layout finds this pane after the code changes; left
/// out, it is the pane's place under its parent (`stage` for the stage).
/// `bare` hides the tab bar: the pane holds one tab and takes no tab dropped onto it, only a split
/// at its edge (a joined dock's single pane uses it). A pane with an `empty` view is never closed:
/// emptied, it shows that view, under its bar's grip unless `hide_empty_bar` drops the bar while it
/// is empty. `locked` makes it an area that keeps its tabs (`DockState::home`); every pane of a dock
/// with a stage is locked.
///
/// `stage` marks an editor's stage: bare, holding one tab (untitled, as its dock names it after the
/// tab it joined), focused when the dock starts, and what the gathered mode keeps
/// (`DockState::stage_of`). `chrome` marks a pane the frame hides with the rest of its chrome
/// (`provide_dock_chrome`), as a split leaves out a child it no longer has.
#[component]
pub fn DockPane(
    id: Option<String>,
    active: Option<DockId>,
    #[prop(default = false)] vertical: bool,
    #[prop(default = false)] icons_only: bool,
    #[prop(default = SIDEBAR_WIDTH)] sidebar_width: f32,
    #[prop(default = false)] locked: bool,
    #[prop(default = false)] bare: bool,
    #[prop(default = false)] stage: bool,
    #[prop(default = false)] chrome: bool,
    #[prop(default = false)] hide_empty_bar: bool,
    empty: Option<BuildFn<NodeId>>,
    children: Children<DockEntry>,
) -> DockNode {
    DockNode {
        kind: NodeKind::Pane {
            key: Keyed::new(id),
            active,
            vertical,
            icons_only,
            sidebar: sidebar_width,
            locked,
            bare: bare || stage,
            stage,
            chrome,
            hide_empty_bar,
            empty,
            entries: children.into_run(),
        },
    }
}

/// A floating window holding `children`. `hidden` opens it hidden and unfocused the first time
/// it is added, as `DockState::hide_window` leaves a window: placed nowhere until one of its tabs
/// is shown; a window already there keeps whether it shows.
#[component]
pub fn DockWindow(
    id: String,
    rect: Rect,
    #[prop(default = false)] hidden: bool,
    children: Children<DockNode>,
) -> DockNode {
    DockNode {
        kind: NodeKind::Window {
            key: id,
            rect,
            hidden,
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
    #[prop(children)] content: BuildFn<NodeId>,
) -> DockNode
where
    K: DockKey,
{
    DockNode {
        kind: NodeKind::Dialog {
            id: DockId::of(&id),
            parent: create_memo(move || DockId::of(&parent.get())),
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
    #[prop(children)] content: BuildFn<NodeId>,
) -> DockNode
where
    K: DockKey,
{
    DockNode {
        kind: NodeKind::Bar {
            id: DockId::of(&id),
            parent: parent.as_ref().map(DockId::of),
            placement,
            content,
        },
    }
}

/// A tab is closed when it stops being declared, even for a moment (a `Show` that flips, a
/// `ForEach` rebuilt from new keys), losing its place, history and panel. `on_close` only gives it a
/// close button and asks the caller to stop declaring it.
///
/// `id` is any `DockKey`; left out, it is derived from the tab's title and its place among its
/// pane's tabs of that title. Give one to a tab the code shows, reads or saves by key.
///
/// In a dock with a stage, the body of a tab outside a bare pane is the standard panel: a
/// `Surface` fill, a `Scroll` filling it and padding inside that, around the content. `bare` opts
/// out, for a body that fills the panel itself.
///
/// `back_closes=false` keeps back from closing a tab it opened (a program's window, whose close ends
/// the program). `see_through` shows what is drawn under the dock, so nothing fills behind it and it
/// never floats into a window. `group` names which tabs are one app's, for `next_of_group`.
/// `picture` is not carried across apps.
#[component]
pub fn DockTab(
    id: Option<DockId>,
    #[prop(default = String::new())] title: Prop<String>,
    #[prop(default = None)] icon: Prop<Option<Glyph>>,
    #[prop(default = None)] picture: Prop<Option<Image>>,
    on_close: Option<Callback<()>>,
    #[prop(default = true)] back_closes: bool,
    #[prop(default = false)] see_through: bool,
    #[prop(default = false)] bare: bool,
    #[prop(default = None)] group: Prop<Option<String>>,
    #[prop(children)] content: BuildFn<NodeId>,
) -> DockEntry {
    DockEntry {
        kind: EntryKind::Tab {
            id: Keyed::new(id),
            title: create_memo(move || title.get()),
            icon: create_memo(move || icon.get()),
            picture: create_memo(move || picture.get()),
            on_close,
            back_closes,
            see_through,
            bare,
            group: create_memo(move || group.get()),
            content,
        },
    }
}

#[component]
pub fn DockGroup(
    id: String,
    title: Option<Prop<String>>,
    #[prop(default = false)] pinned: bool,
    children: Children<DockNode>,
) -> DockEntry {
    DockEntry {
        kind: EntryKind::Group {
            key: id,
            title: title.map(|title| create_memo(move || title.get())),
            pinned,
            children: children.into_run(),
        },
    }
}
