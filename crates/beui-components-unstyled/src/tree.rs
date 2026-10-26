use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::button::{Button, ButtonHandle};
use crate::context_menu::ContextMenu;
use crate::floating::{Edge, Floating};
use crate::menu::MenuItem;
use crate::menu_button::MenuButton;
use crate::menu_popup::MenuStyle;
use crate::popup_trigger::PopupTriggerHandle;
use crate::text_input::{TextInput, TextInputHandle, TextInputStyle};
use crate::typeahead::Typeahead;
use beui_core::geometry::{Pos2, Rect};
use beui_core::input::{CursorIcon, Key, KeyPress, PointerPress};
use beui_core::keys::{KeyAt, KeyPhase};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const ROW_DRAG_THRESHOLD: f32 = 6.0;

#[derive(Clone, Default, PartialEq)]
pub struct TreeItem {
    pub label: String,
    pub depth: usize,
    pub expandable: bool,
    pub expanded: bool,
}

pub struct TreeMenu {
    pub items: Children<MenuItem>,
    pub on_select: Callback<Vec<usize>>,
}

impl ChildValue for TreeMenu {}

#[derive(Clone)]
pub struct TreeRowMenu {
    items: Run<MenuItem>,
    on_select: Callback<Vec<usize>>,
    style: MenuStyle,
}

pub struct TreeRowHandle<K> {
    pub key: K,
    pub item: Memo<TreeItem>,
    pub selected: Memo<bool>,
    pub marked: Memo<bool>,
    pub focused: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub select: ClickCallback,
    pub toggle: ClickCallback,
    pub target: TreeRowTarget,
    pub editing: Memo<bool>,
    pub edit: TreeRowEdit,
    pub menu: Option<TreeRowMenu>,
}

#[derive(Clone, Copy)]
pub struct TreeRowEdit {
    item: Memo<TreeItem>,
    finish: Callback<Finish>,
}

enum Finish {
    Commit(String),
    Leave(String),
    Cancel,
}

#[derive(Clone, Copy)]
pub struct TreeRowTarget {
    select: ClickCallback,
    open: ClickCallback,
    drag_start: ClickCallback,
    hover: Callback<bool>,
    hovered: Signal<bool>,
    active: Signal<bool>,
}

pub struct TreeRevealHandle {
    pub edge: Memo<Edge>,
    pub label: Memo<String>,
    pub reveal: ClickCallback,
}

struct State<K> {
    list: NodeRef,
    nodes: Nodes<K>,
    keys: Memo<Vec<K>>,
    item: Func<K, TreeItem>,
    selected: Memo<Option<K>>,
    shown: Memo<Option<K>>,
    buried: Memo<Option<K>>,
    focus: Signal<Option<K>>,
    on_select: Callback<K>,
    on_open: Callback<K>,
    on_expand: Callback<(K, bool)>,
    on_edit: Callback<K>,
    on_rename: Callback<(K, String)>,
    on_cancel_edit: Callback<K>,
    menu: Option<Func<K, Option<TreeMenu>>>,
    menu_style: MenuStyle,
    typeahead: Stored<RefCell<Typeahead>>,
}

impl<K> Clone for State<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for State<K> {}

type Nodes<K> = Stored<RefCell<HashMap<K, NodeRef>>>;

#[component]
pub fn Tree<K>(
    #[prop(default = ComponentRef::new())] api: ComponentRef<TreeApi<K>>,
    keys: Prop<Vec<K>>,
    item: Func<K, TreeItem>,
    selected: Prop<Option<K>>,
    #[prop(default = None)] ancestors: Option<Func<K, Vec<K>>>,
    #[prop(default = 0.0)] spacing: f32,
    #[prop(default = None)] editing: Prop<Option<K>>,
    #[prop(default = None)] menu: Option<Func<K, Option<TreeMenu>>>,
    #[prop(default = MenuStyle::default())] menu_style: MenuStyle,
    on_select: Callback<K>,
    on_open: Callback<K>,
    on_expand: Callback<(K, bool)>,
    on_hover_change: Callback<(K, bool)>,
    on_drag_start: Callback<K>,
    on_edit: Callback<K>,
    on_rename: Callback<(K, String)>,
    on_cancel_edit: Callback<K>,
    #[prop(children)] row: Option<RenderFn<TreeRowHandle<K>>>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
{
    let row = row.expect("tree requires a `row` builder");
    let keys = create_memo(move || keys.get());
    let selected = create_memo(move || selected.get());
    let selection = create_selector(move || selected.get());
    let ancestors = ancestors.unwrap_or_else(|| Func::new(|_| Vec::new()));
    let shown = create_memo(move || {
        let selected = selected.get()?;
        let lineage = ancestors.call(selected.clone());
        keys.with(|keys| {
            std::iter::once(selected)
                .chain(lineage.into_iter().rev())
                .find(|key| keys.contains(key))
        })
    });
    let buried = create_memo(move || {
        shown
            .get()
            .filter(|key| selected.get().as_ref() != Some(key))
    });
    let burial = create_selector(move || buried.get());
    let focus = create_signal(None);
    let focused = create_selector(move || focus.get());
    let tab_stop = create_memo(move || {
        let keys = keys.get();
        let present = |key: Option<K>| key.filter(|key| keys.contains(key));
        present(focus.get())
            .or_else(|| present(selected.get()))
            .or_else(|| keys.first().cloned())
    });
    let tab_stops = create_selector(move || tab_stop.get());
    let edited = create_selector(move || editing.get());

    component_accessibility(Node::new(Role::Tree));

    let nodes: Nodes<K> = store(RefCell::default());
    let list = NodeRef::new();
    let state = State {
        list,
        nodes,
        keys,
        item,
        selected,
        shown,
        buried,
        focus,
        on_select,
        on_open,
        on_expand,
        on_edit,
        on_rename,
        on_cancel_edit,
        menu,
        menu_style,
        typeahead: store(RefCell::default()),
    };
    api.fill(TreeApi(state));

    create_effect(move || {
        if state.focus.get().is_none() {
            state.typeahead.value().borrow_mut().clear();
        }
    });

    view! {
        <List spacing @node_ref=&list>
            <ForEach keys>|key: K| {
                let item = create_memo({ let key = key.clone(); move || item.call(key.clone()) });
                let selected = selection.memo(Some(key.clone()));
                let marked = burial.memo(Some(key.clone()));
                let editing = edited.memo(Some(key.clone()));
                view! {
                    <TreeRow
                        row_key={key}
                        item
                        selected
                        marked
                        editing
                        tab_stop={tab_stops}
                        focused={focused}
                        state={state}
                        nodes={nodes}
                        row={row}
                        on_hover_change={on_hover_change}
                        on_drag_start={on_drag_start}
                    />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn TreeRow<K>(
    row_key: K,
    item: Memo<TreeItem>,
    selected: Memo<bool>,
    marked: Memo<bool>,
    editing: Memo<bool>,
    tab_stop: Selector<Option<K>>,
    focused: Selector<Option<K>>,
    state: State<K>,
    nodes: Nodes<K>,
    row: RenderFn<TreeRowHandle<K>>,
    on_hover_change: Callback<(K, bool)>,
    on_drag_start: Callback<K>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
{
    let accessibility = create_memo(move || {
        let item = item.get();
        let mut node = Node::new(Role::TreeItem);
        node.set_label(item.label);
        node.set_level(item.depth + 1);
        if item.expandable {
            node.set_expanded(item.expanded);
        }
        node.set_selected(selected.get());
        node
    });
    component_accessibility(accessibility);
    let (blur_key, press_key, typed_key, content_key) = (
        row_key.clone(),
        row_key.clone(),
        row_key.clone(),
        row_key.clone(),
    );
    let (nodes_key, cleanup_key) = (row_key.clone(), row_key.clone());
    let (select_key, toggle_key, open_key) = (row_key.clone(), row_key.clone(), row_key.clone());
    let chosen = ClickCallback::new(move || select(state, &select_key));
    let opened = ClickCallback::new(move || state.on_open.call(open_key.clone()));
    let expand = ClickCallback::new(move || toggle(state, &toggle_key));
    let hover_key = row_key.clone();
    let hover = Callback::new(move |over: bool| {
        on_hover_change.call((hover_key.clone(), over));
    });
    let drag_key = row_key.clone();
    let drag_start = ClickCallback::new(move || on_drag_start.call(drag_key.clone()));
    let hovered = create_signal(false);
    let active = create_signal(false);
    let target = TreeRowTarget {
        select: chosen,
        open: opened,
        drag_start,
        hover,
        hovered,
        active,
    };
    let edit = TreeRowEdit {
        item,
        finish: Callback::new({
            let key = row_key.clone();
            move |finish: Finish| finish_edit(state, &key, item, finish)
        }),
    };
    let menu = state.menu.and_then(|menu| menu.call(row_key.clone())).map(
        |TreeMenu { items, on_select }| TreeRowMenu {
            items: items.into_run(),
            on_select,
            style: state.menu_style,
        },
    );
    let has_focus = create_signal(false);
    let built = NodeRef::new();
    nodes.value().borrow_mut().insert(nodes_key, built);
    on_cleanup(move || {
        nodes.try_with(|nodes| nodes.borrow_mut().remove(&cleanup_key));
    });
    let face = row.call(TreeRowHandle {
        key: content_key,
        item,
        selected,
        marked,
        focused: has_focus.read_only(),
        hovered: hovered.read_only(),
        active: active.read_only(),
        select: chosen,
        toggle: expand,
        target,
        editing,
        edit,
        menu: menu.clone(),
    });
    let content = match menu {
        None => face,
        Some(TreeRowMenu {
            items,
            on_select,
            style,
        }) => view! {
            <ContextMenu items={Children::from(items)} menu=style on_select>{face}</ContextMenu>
        },
    };
    view! {
        <Interactive
            @node_ref=&built
            focusable=true
            tab_stop={tab_stop.memo(Some(row_key.clone()))}
            focused={focused.memo(Some(row_key))}
            on_focus_change={move |focused: bool| {
                has_focus.set(focused);
                track_focus(state, &blur_key, focused);
            }}
            on_activate={chosen}
            on_key={move |event: KeyPress| key_press(state, &press_key, event)}
            on_text={move |text: String| typeahead(state, &typed_key, &text)}
        >
            {content}
        </Interactive>
    }
}

#[component]
pub fn TreeRowMenuButton(
    menu: TreeRowMenu,
    #[prop(default = String::from("More actions"))] label: Prop<String>,
    #[prop(children)] trigger: Render<PopupTriggerHandle>,
) -> NodeId {
    let TreeRowMenu {
        items,
        on_select,
        style,
    } = menu;
    view! {
        <MenuButton
            items={Children::from(items)}
            trigger
            menu=style
            label
            icon_only=true
            tab_stop=false
            capture_presses=true
            on_select
        />
    }
}

pub struct TreeToggleHandle {
    pub button: ButtonHandle,
    pub expanded: Memo<bool>,
}

#[component]
pub fn TreeToggle(
    item: Memo<TreeItem>,
    toggle: ClickCallback,
    #[prop(default = String::new())] button_test_id: String,
    #[prop(children)] content: RenderFn<TreeToggleHandle>,
) -> DynamicSegment<ListChild> {
    let expandable = create_memo(move || item.get().expandable);
    let expanded = create_memo(move || item.get().expanded);
    view! {
        <Show condition={expandable}>|| {
            let button_test_id = button_test_id.clone(); {
                let label = create_memo(move || match expanded.get() {
                    true => "Collapse".to_owned(),
                    false => "Expand".to_owned(),
                });
                let accessibility = create_memo(move || {
                    let mut node = Node::new(Role::Button);
                    node.set_expanded(expanded.get());
                    node
                });
                view! {
                    <Button
                        @test_id={button_test_id}
                        label
                        tab_stop=false
                        press_focus=false
                        accessibility
                        on_click={toggle}
                        content={move |button: ButtonHandle| content.call(TreeToggleHandle {
                            button,
                            expanded,
                        })}
                    />
                }
            }
        }</Show>
    }
}

#[component]
pub fn TreeRowArea(target: TreeRowTarget, children: Child) -> NodeId {
    let TreeRowTarget {
        select,
        open,
        drag_start,
        hover,
        hovered,
        active,
    } = target;
    let gesture: Stored<Cell<Option<(Pos2, bool)>>> = store(Cell::default());
    let pressed = move |press: PointerPress| {
        gesture.with(|gesture| gesture.set(Some((press.pos, false))));
        if press.clicks >= 2 {
            open.call();
        }
    };
    let dragged = move |at: PointerPress| {
        let Some((origin, started)) = gesture.with(Cell::get) else {
            return;
        };
        if started || (at.pos - origin).length() < ROW_DRAG_THRESHOLD {
            return;
        }
        gesture.with(|gesture| gesture.set(Some((origin, true))));
        drag_start.call();
    };
    let settled = move |down: bool| {
        active.set(down);
        if !down {
            gesture.with(|gesture| gesture.set(None));
        }
    };
    let chose = move || {
        if gesture.with(Cell::get).is_none_or(|(_, started)| !started) {
            select.call();
        }
    };
    view! {
        <Interactive
            cursor=CursorIcon::PointingHand
            on_press={pressed}
            on_drag={dragged}
            on_click={chose}
            on_active_change={settled}
            on_hover_change={move |over: bool| {
                hovered.set(over);
                hover.call(over);
            }}
        >
            {children}
        </Interactive>
    }
}

#[component]
pub fn TreeRowEditor(
    edit: TreeRowEdit,
    #[prop(default = String::from("Name"))] label: Prop<String>,
    #[prop(default = TextInputStyle::default())] style: TextInputStyle,
    #[prop(children)] content: Render<TextInputHandle>,
) -> NodeId {
    let TreeRowEdit { item, finish } = edit;
    let initial = item.get_untracked().label;
    let typed = Rc::new(RefCell::new(initial.clone()));
    let done = Rc::new(Cell::new(false));
    let once = move |ending: Finish| {
        if !done.replace(true) {
            finish.call(ending);
        }
    };
    let field = NodeRef::new();
    let escape = once.clone();
    on_key(KeyAt::Node(field), KeyPhase::Capture, move |event| {
        let escaped = event
            .press()
            .is_some_and(|press| press.pressed && press.key == Key::Escape);
        if escaped {
            escape(Finish::Cancel);
        }
        escaped
    });
    let placed = create_memo(move || field.try_get().is_some_and(|node| node_placed(node).get()));
    create_effect(move || {
        if placed.get() {
            field.get().reveal();
        }
    });
    let keyboard = use_keyboard();
    let held = Cell::new(keyboard.get_untracked());
    let (away, left) = (once.clone(), typed.clone());
    create_effect(move || {
        let now = keyboard.get();
        if held.replace(now) && !now {
            away(Finish::Leave(left.borrow().clone()));
        }
    });
    let (submit, leave, changed) = (once.clone(), once, typed.clone());
    view! {
        <TextInput
            @node_ref=&field
            value={Binding::new(initial, move |text: String| *changed.borrow_mut() = text)}
            placeholder={String::new()}
            label
            focused=true
            select_on_focus=true
            style
            content
            on_submit={move |text: String| submit(Finish::Commit(text))}
            on_focus_change={move |focused: bool| {
                if !focused {
                    leave(Finish::Leave(typed.borrow().clone()));
                }
            }}
        />
    }
}

#[component]
pub fn TreeReveal<K>(
    tree: ComponentRef<TreeApi<K>>,
    viewport: NodeRef,
    on_reveal: Callback<K>,
    button: RenderFn<TreeRevealHandle>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
{
    let state = tree.get().0;
    let (list, scroll) = (state.list.get(), viewport.get());
    let (keys, shown, buried, selected) = (state.keys, state.shown, state.buried, state.selected);
    let astray = create_memo(move || {
        let key = shown.get()?;
        keys.with(|_| ());
        node_rect(list).get();
        let row = state
            .nodes
            .value()
            .borrow()
            .get(&key)
            .and_then(NodeRef::try_get)?;
        if !node_placed(row).get() || !node_placed(scroll).get() {
            return None;
        }
        stray_edge(
            node_rect(row).get(),
            node_rect(scroll).get(),
            buried.get().is_some(),
        )
    });
    let pending = create_signal(None::<K>);
    create_effect(move || {
        let Some(key) = pending.get() else {
            return;
        };
        keys.with(|_| ());
        node_rect(list).get();
        let Some(row) = state
            .nodes
            .value()
            .borrow()
            .get(&key)
            .and_then(NodeRef::try_get)
        else {
            return;
        };
        if !node_placed(row).get() {
            return;
        }
        row.reveal();
        pending.set(None);
    });
    let open = create_memo(move || astray.get().is_some());
    let edge = create_memo(move || astray.get().unwrap_or(Edge::Bottom));
    let item = state.item;
    let label = create_memo(move || {
        let label = selected
            .get()
            .map(|key| item.call(key).label)
            .unwrap_or_default();
        match label.is_empty() {
            true => "Show the selection".to_owned(),
            false => label,
        }
    });
    let reveal = ClickCallback::new(move || {
        let Some(key) = selected.get_untracked() else {
            return;
        };
        on_reveal.call(key.clone());
        pending.set(Some(key));
    });
    let content = RenderFn::new(move |()| {
        button.call(TreeRevealHandle {
            edge,
            label,
            reveal,
        })
    });
    view! {
        <Floating anchor={viewport} edge={edge} open={open} content={move || content.call(())} />
    }
}

fn stray_edge(row: Rect, viewport: Rect, buried: bool) -> Option<Edge> {
    if row.bottom() <= viewport.top() {
        return Some(Edge::Top);
    }
    if row.top() >= viewport.bottom() || buried {
        return Some(Edge::Bottom);
    }
    None
}

fn track_focus<K>(state: State<K>, key: &K, has_focus: bool)
where
    K: Clone + Eq + Hash + 'static,
{
    if has_focus {
        state.focus.set(Some(key.clone()));
    } else if state.focus.get_untracked().as_ref() == Some(key) {
        state.focus.set(None);
    }
}

fn finish_edit<K>(state: State<K>, key: &K, item: Memo<TreeItem>, finish: Finish)
where
    K: Clone + Eq + Hash + 'static,
{
    let (typed, refocus) = match finish {
        Finish::Commit(typed) => (Some(typed), true),
        Finish::Leave(typed) => (Some(typed), false),
        Finish::Cancel => (None, true),
    };
    let label = untrack(|| item.get().label);
    let renamed = typed
        .map(|typed| typed.trim().to_owned())
        .filter(|name| !name.is_empty() && *name != label);
    match renamed {
        Some(name) => state.on_rename.call((key.clone(), name)),
        None => state.on_cancel_edit.call(key.clone()),
    }
    if refocus {
        state.focus.set(Some(key.clone()));
    }
}

fn toggle<K>(state: State<K>, key: &K)
where
    K: Clone + Eq + Hash + 'static,
{
    state.focus.set(Some(key.clone()));
    let item = untrack(|| state.item.call(key.clone()));
    if item.expandable {
        state.on_expand.call((key.clone(), !item.expanded));
    }
}

fn key_press<K>(state: State<K>, key: &K, press: KeyPress) -> bool
where
    K: Clone + Eq + Hash + 'static,
{
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let keys = state.keys.get_untracked();
    let Some(index) = keys.iter().position(|candidate| candidate == key) else {
        return false;
    };
    let item = untrack(|| state.item.call(key.clone()));
    let moved = match press.key {
        Key::ArrowUp => index.saturating_sub(1),
        Key::ArrowDown => (index + 1).min(keys.len() - 1),
        Key::Home => 0,
        Key::End => keys.len() - 1,
        Key::ArrowRight if item.expandable && !item.expanded => {
            if press.pressed {
                state.on_expand.call((key.clone(), true));
            }
            return true;
        }
        Key::ArrowRight if item.expanded => (index + 1).min(keys.len() - 1),
        Key::ArrowRight => return true,
        Key::ArrowLeft if item.expandable && item.expanded => {
            if press.pressed {
                state.on_expand.call((key.clone(), false));
            }
            return true;
        }
        Key::ArrowLeft => match parent(state, &keys, index, item.depth) {
            Some(parent) => parent,
            None => return true,
        },
        Key::Enter if !state.on_open.is_empty() => {
            if press.pressed {
                select(state, key);
                state.on_open.call(key.clone());
            }
            return true;
        }
        Key::F2 if !state.on_edit.is_empty() => {
            if press.pressed {
                state.on_edit.call(key.clone());
            }
            return true;
        }
        _ => return false,
    };
    if press.pressed {
        state.focus.set(Some(keys[moved].clone()));
    }
    true
}

fn parent<K>(state: State<K>, keys: &[K], index: usize, depth: usize) -> Option<usize>
where
    K: Clone + Eq + Hash + 'static,
{
    keys[..index]
        .iter()
        .rposition(|key| untrack(|| state.item.call(key.clone())).depth < depth)
}

fn typeahead<K>(state: State<K>, key: &K, text: &str)
where
    K: Clone + Eq + Hash + 'static,
{
    let keys = state.keys.get_untracked();
    let Some(index) = keys.iter().position(|candidate| candidate == key) else {
        return;
    };
    let matched = state
        .typeahead
        .value()
        .borrow_mut()
        .matched(text, index, keys.len(), |row| {
            untrack(|| state.item.call(keys[row].clone())).label
        });
    if let Some(matched) = matched {
        state.focus.set(Some(keys[matched].clone()));
    }
}

fn select<K>(state: State<K>, key: &K)
where
    K: Clone + Eq + Hash + 'static,
{
    state.focus.set(Some(key.clone()));
    state.on_select.call(key.clone());
}

pub struct TreeApi<K: 'static>(State<K>);

impl<K> Clone for TreeApi<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for TreeApi<K> {}

impl<K: Clone + Eq + Hash + 'static> TreeApi<K> {
    pub fn row_node(&self, key: &K) -> Option<NodeId> {
        self.0
            .nodes
            .with(|nodes| nodes.borrow().get(key).and_then(NodeRef::try_get))
    }

    pub fn focused(&self) -> Option<K> {
        self.0.focus.get()
    }

    pub fn node(&self) -> NodeId {
        self.0.list.get()
    }
}
