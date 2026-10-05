use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::button::{Button, ButtonHandle};
use crate::floating::{Edge, Floating};
use crate::typeahead::Typeahead;
use beui_core::document::Document;
use beui_core::geometry::{Pos2, Rect};
use beui_core::input::{CursorIcon, Key, KeyPress, PointerPress};
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, Child, ClickCallback, DynamicSegment, ForEach, Func, Interactive, List, ListChild,
    Memo, NodeRef, Prop, ReadSignal, Render, RenderFn, Selector, Show, WriteSignal, clone,
    component_accessibility, create_effect, create_memo, create_selector, create_signal,
    node_placed, node_rect, on_cleanup, set_component_state, untrack, with_document,
};

const ROW_DRAG_THRESHOLD: f32 = 6.0;

#[derive(Clone, Default, PartialEq)]
pub struct TreeItem {
    pub label: String,
    pub depth: usize,
    pub expandable: bool,
    pub expanded: bool,
}

pub struct TreeRowHandle<K> {
    pub key: K,
    pub item: Memo<TreeItem>,
    pub selected: Memo<bool>,
    pub marked: Memo<bool>,
    pub focused: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub select: Rc<dyn Fn()>,
    pub toggle: Rc<dyn Fn()>,
    pub target: TreeRowTarget,
}

#[derive(Clone)]
pub struct TreeRowTarget {
    select: Rc<dyn Fn()>,
    drag_start: Rc<dyn Fn()>,
    hover: Rc<dyn Fn(bool)>,
    set_hovered: WriteSignal<bool>,
    set_active: WriteSignal<bool>,
}

pub struct TreeRevealHandle {
    pub edge: Memo<Edge>,
    pub label: Memo<String>,
    pub reveal: ClickCallback,
}

struct State<K> {
    nodes: Nodes<K>,
    keys: Memo<Vec<K>>,
    item: Func<K, TreeItem>,
    selected: Memo<Option<K>>,
    shown: Memo<Option<K>>,
    buried: Memo<Option<K>>,
    focus: ReadSignal<Option<K>>,
    set_focus: WriteSignal<Option<K>>,
    selection_follows_focus: bool,
    on_select: Callback<K>,
    on_expand: Callback<(K, bool)>,
    typeahead: RefCell<Typeahead>,
}

type Handle<K> = Rc<State<K>>;

type Nodes<K> = Rc<RefCell<HashMap<K, NodeRef>>>;

#[component]
pub fn Tree<K>(
    keys: Prop<Vec<K>>,
    item: Func<K, TreeItem>,
    selected: Prop<Option<K>>,
    #[prop(default = None)] ancestors: Option<Func<K, Vec<K>>>,
    #[prop(default = 0.0)] spacing: f32,
    #[prop(default = false)] selection_follows_focus: bool,
    on_select: Callback<K>,
    on_expand: Callback<(K, bool)>,
    on_hover_change: Callback<(K, bool)>,
    on_drag_start: Callback<K>,
    #[prop(children)] row: Option<RenderFn<TreeRowHandle<K>>>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
{
    let row = row.expect("tree requires a `row` builder");
    let keys = create_memo(move || keys.get());
    let selected = create_memo(move || selected.get());
    let selection = create_selector(clone!(selected -> move || selected.get()));
    let ancestors = ancestors.unwrap_or_else(|| Func::new(|_| Vec::new()));
    let shown = create_memo(clone!(keys selected -> move || {
        let selected = selected.get()?;
        let lineage = ancestors.call(selected.clone());
        keys.with(|keys| {
            std::iter::once(selected)
                .chain(lineage.into_iter().rev())
                .find(|key| keys.contains(key))
        })
    }));
    let buried = create_memo(clone!(shown selected -> move || {
        shown.get().filter(|key| selected.get().as_ref() != Some(key))
    }));
    let burial = create_selector(clone!(buried -> move || buried.get()));
    let (focus, set_focus) = create_signal(None);
    let focused = create_selector(clone!(focus -> move || focus.get()));
    let tab_stop = create_memo(clone!(keys selected focus -> move || {
        let keys = keys.get();
        let present = |key: Option<K>| key.filter(|key| keys.contains(key));
        present(focus.get())
            .or_else(|| present(selected.get()))
            .or_else(|| keys.first().cloned())
    }));
    let tab_stops = create_selector(clone!(tab_stop -> move || tab_stop.get()));

    component_accessibility(Node::new(Role::Tree));

    let nodes: Nodes<K> = Rc::default();
    let state: Handle<K> = Rc::new(State {
        nodes: nodes.clone(),
        keys: keys.clone(),
        item: item.clone(),
        selected: selected.clone(),
        shown,
        buried,
        focus: focus.clone(),
        set_focus,
        selection_follows_focus,
        on_select,
        on_expand,
        typeahead: RefCell::default(),
    });
    set_component_state(state.clone());

    create_effect(clone!(state -> move || {
        if state.focus.get().is_none() {
            state.typeahead.borrow_mut().clear();
        }
    }));

    view! {
        <List spacing>
            <ForEach keys>
                {move |key: K| {
                    let item = create_memo(clone!(item key -> move || item.call(key.clone())));
                    let selected = selection.memo(Some(key.clone()));
                    let marked = burial.memo(Some(key.clone()));
                    let hover = on_hover_change.clone();
                    let dragging = on_drag_start.clone();
                    view! {
                        <TreeRow
                            row_key={key}
                            item
                            selected
                            marked
                            tab_stop={tab_stops.clone()}
                            focused={focused.clone()}
                            state={state.clone()}
                            nodes={nodes.clone()}
                            row={row.clone()}
                            on_hover_change={move |change| hover.call(change)}
                            on_drag_start={move |key| dragging.call(key)}
                        />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn TreeRow<K>(
    row_key: K,
    item: Memo<TreeItem>,
    selected: Memo<bool>,
    marked: Memo<bool>,
    tab_stop: Selector<Option<K>>,
    focused: Selector<Option<K>>,
    state: Handle<K>,
    nodes: Nodes<K>,
    row: RenderFn<TreeRowHandle<K>>,
    on_hover_change: Callback<(K, bool)>,
    on_drag_start: Callback<K>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
{
    let key = row_key;
    let accessibility = create_memo(clone!(item selected -> move || {
        let item = item.get();
        let mut node = Node::new(Role::TreeItem);
        node.set_label(item.label);
        node.set_level(item.depth + 1);
        if item.expandable {
            node.set_expanded(item.expanded);
        }
        node.set_selected(selected.get());
        node
    }));
    component_accessibility(accessibility);
    let (blur, press, typed) = (state.clone(), state.clone(), state.clone());
    let (blur_key, press_key, typed_key, content_key) =
        (key.clone(), key.clone(), key.clone(), key.clone());
    let (nodes_key, cleanup_key) = (key.clone(), key.clone());
    let (selecting, toggling) = (state.clone(), state.clone());
    let (select_key, toggle_key) = (key.clone(), key.clone());
    let chosen: Rc<dyn Fn()> = Rc::new(move || select(&selecting, &select_key));
    let expand: Rc<dyn Fn()> = Rc::new(move || toggle(&toggling, &toggle_key));
    let hover_key = key.clone();
    let hover: Rc<dyn Fn(bool)> = Rc::new(move |over: bool| {
        on_hover_change.call((hover_key.clone(), over));
    });
    let drag_key = key.clone();
    let drag_start: Rc<dyn Fn()> = Rc::new(move || on_drag_start.call(drag_key.clone()));
    let (hovered, set_hovered) = create_signal(false);
    let (active, set_active) = create_signal(false);
    let target = TreeRowTarget {
        select: chosen.clone(),
        drag_start,
        hover,
        set_hovered,
        set_active,
    };
    let activating = chosen.clone();
    let (has_focus, set_has_focus) = create_signal(false);
    let built = NodeRef::new();
    nodes.borrow_mut().insert(nodes_key, built.clone());
    on_cleanup(move || {
        nodes.borrow_mut().remove(&cleanup_key);
    });
    view! {
        <Interactive
            @node_ref=&built
            focusable=true
            tab_stop={tab_stop.memo(Some(key.clone()))}
            focused={focused.memo(Some(key))}
            on_focus_change={move |focused: bool| {
                set_has_focus.set(focused);
                track_focus(&blur, &blur_key, focused);
            }}
            on_activate={move || activating()}
            on_key={move |event: KeyPress| key_press(&press, &press_key, event)}
            on_text={move |text: String| typeahead(&typed, &typed_key, &text)}
        >
            {row.call(TreeRowHandle {
                key: content_key,
                item,
                selected,
                marked,
                focused: has_focus,
                hovered,
                active,
                select: chosen,
                toggle: expand,
                target,
            })}
        </Interactive>
    }
}

pub struct TreeToggleHandle {
    pub button: ButtonHandle,
    pub expanded: Memo<bool>,
}

#[component]
pub fn TreeToggle(
    item: Memo<TreeItem>,
    toggle: Rc<dyn Fn()>,
    #[prop(default = String::new())] button_test_id: String,
    #[prop(children)] content: RenderFn<TreeToggleHandle>,
) -> DynamicSegment<ListChild> {
    let expandable = create_memo(clone!(item -> move || item.get().expandable));
    let expanded = create_memo(move || item.get().expanded);
    view! {
        <Show condition={expandable}>
            {move || clone!(expanded toggle content button_test_id -> {
                let label = create_memo(clone!(expanded -> move || match expanded.get() {
                    true => "Collapse".to_owned(),
                    false => "Expand".to_owned(),
                }));
                let accessibility = create_memo(clone!(expanded -> move || {
                    let mut node = Node::new(Role::Button);
                    node.set_expanded(expanded.get());
                    node
                }));
                view! {
                    <Button
                        @test_id={button_test_id}
                        label
                        tab_stop=false
                        press_focus=false
                        accessibility
                        on_click={move || toggle()}
                        content={move |button: ButtonHandle| content.call(TreeToggleHandle {
                            button,
                            expanded: expanded.clone(),
                        })}
                    />
                }
            })}
        </Show>
    }
}

#[component]
pub fn TreeRowArea(target: TreeRowTarget, children: Child) -> NodeId {
    let TreeRowTarget {
        select,
        drag_start,
        hover,
        set_hovered,
        set_active,
    } = target;
    let gesture: Rc<Cell<Option<(Pos2, bool)>>> = Rc::default();
    let pressed = clone!(gesture -> move |press: PointerPress| {
        gesture.set(Some((press.pos, false)));
    });
    let dragged = clone!(gesture -> move |at: PointerPress| {
        let Some((origin, started)) = gesture.get() else {
            return;
        };
        if started || (at.pos - origin).length() < ROW_DRAG_THRESHOLD {
            return;
        }
        gesture.set(Some((origin, true)));
        drag_start();
    });
    let settled = clone!(gesture -> move |down: bool| {
        set_active.set(down);
        if !down {
            gesture.set(None);
        }
    });
    let chose = move || {
        if gesture.get().is_none_or(|(_, started)| !started) {
            select();
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
                set_hovered.set(over);
                hover(over);
            }}
        >
            {children}
        </Interactive>
    }
}

#[component]
pub fn TreeReveal<K>(
    tree: NodeRef,
    viewport: NodeRef,
    on_reveal: Callback<K>,
    button: Render<TreeRevealHandle>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
{
    let (list, scroll) = (tree.get(), viewport.get());
    let state = with_document(|document| document.component_state::<Handle<K>>(list).clone());
    let (keys, shown, buried, selected) = (
        state.keys.clone(),
        state.shown.clone(),
        state.buried.clone(),
        state.selected.clone(),
    );
    let astray = create_memo(clone!(state shown buried keys -> move || {
        let key = shown.get()?;
        keys.with(|_| ());
        node_rect(list).get();
        let row = state.nodes.borrow().get(&key).and_then(NodeRef::try_get)?;
        if !node_placed(row).get() || !node_placed(scroll).get() {
            return None;
        }
        stray_edge(node_rect(row).get(), node_rect(scroll).get(), buried.get().is_some())
    }));
    let (pending, set_pending) = create_signal(None::<K>);
    create_effect(clone!(state keys set_pending -> move || {
        let Some(key) = pending.get() else {
            return;
        };
        keys.with(|_| ());
        node_rect(list).get();
        let Some(row) = state.nodes.borrow().get(&key).and_then(NodeRef::try_get) else {
            return;
        };
        if !node_placed(row).get() {
            return;
        }
        with_document(|document| document.reveal_node(row));
        set_pending.set(None);
    }));
    let open = create_memo(clone!(astray -> move || astray.get().is_some()));
    let edge = create_memo(clone!(astray -> move || astray.get().unwrap_or(Edge::Bottom)));
    let item = state.item.clone();
    let label = create_memo(clone!(selected -> move || {
        let label = selected.get().map(|key| item.call(key).label).unwrap_or_default();
        match label.is_empty() {
            true => "Show the selection".to_owned(),
            false => label,
        }
    }));
    let reveal = ClickCallback::new(move || {
        let Some(key) = selected.get_untracked() else {
            return;
        };
        on_reveal.call(key.clone());
        set_pending.set(Some(key));
    });
    let content = button.call(TreeRevealHandle {
        edge: edge.clone(),
        label,
        reveal,
    });
    view! {
        <Floating anchor={viewport} edge={edge} open={open}>{content}</Floating>
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

pub fn tree_row_node<K>(document: &Document, tree: NodeId, key: &K) -> Option<NodeId>
where
    K: Clone + Eq + Hash + 'static,
{
    document
        .component_state::<Handle<K>>(tree)
        .nodes
        .borrow()
        .get(key)
        .and_then(NodeRef::try_get)
}

pub fn tree_focused<K>(document: &Document, tree: NodeId) -> Option<K>
where
    K: Clone + Eq + Hash + 'static,
{
    document.component_state::<Handle<K>>(tree).focus.get()
}

fn track_focus<K>(state: &State<K>, key: &K, has_focus: bool)
where
    K: Clone + Eq + Hash + 'static,
{
    if has_focus {
        state.set_focus.set(Some(key.clone()));
    } else if state.focus.get_untracked().as_ref() == Some(key) {
        state.set_focus.set(None);
    }
}

fn toggle<K>(state: &State<K>, key: &K)
where
    K: Clone + Eq + Hash + 'static,
{
    state.set_focus.set(Some(key.clone()));
    let item = untrack(|| state.item.call(key.clone()));
    if item.expandable {
        state.on_expand.call((key.clone(), !item.expanded));
    }
}

fn key_press<K>(state: &State<K>, key: &K, press: KeyPress) -> bool
where
    K: Clone + Eq + Hash + 'static,
{
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let keys = untrack(|| state.keys.get());
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
        _ => return false,
    };
    if press.pressed {
        walk(state, &keys[moved]);
    }
    true
}

fn parent<K>(state: &State<K>, keys: &[K], index: usize, depth: usize) -> Option<usize>
where
    K: Clone + Eq + Hash + 'static,
{
    keys[..index]
        .iter()
        .rposition(|key| untrack(|| state.item.call(key.clone())).depth < depth)
}

fn typeahead<K>(state: &State<K>, key: &K, text: &str)
where
    K: Clone + Eq + Hash + 'static,
{
    let keys = untrack(|| state.keys.get());
    let Some(index) = keys.iter().position(|candidate| candidate == key) else {
        return;
    };
    let matched = state
        .typeahead
        .borrow_mut()
        .matched(text, index, keys.len(), |row| {
            untrack(|| state.item.call(keys[row].clone())).label
        });
    if let Some(matched) = matched {
        walk(state, &keys[matched]);
    }
}

fn walk<K>(state: &State<K>, key: &K)
where
    K: Clone + Eq + Hash + 'static,
{
    match state.selection_follows_focus {
        true => select(state, key),
        false => state.set_focus.set(Some(key.clone())),
    }
}

fn select<K>(state: &State<K>, key: &K)
where
    K: Clone + Eq + Hash + 'static,
{
    state.set_focus.set(Some(key.clone()));
    state.on_select.call(key.clone());
}
