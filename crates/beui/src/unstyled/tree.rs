use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::NodeId;
use crate::reactive::{
    Callback, ForEach, Func, List, Memo, Prop, ReadSignal, RenderFn, Selector, WriteSignal, clone,
    component_accessibility, create_effect, create_memo, create_selector, create_signal,
    on_cleanup, set_component_state, untrack, with_document,
};
use crate::unstyled;
use crate::unstyled::ButtonHandle;
use crate::unstyled::typeahead::Typeahead;

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
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub toggle: Rc<dyn Fn()>,
}

struct State<K> {
    keys: Memo<Vec<K>>,
    item: Func<K, TreeItem>,
    expand_on_select: Memo<bool>,
    focus: ReadSignal<Option<K>>,
    set_focus: WriteSignal<Option<K>>,
    on_select: Callback<K>,
    on_expand: Callback<(K, bool)>,
    typeahead: RefCell<Typeahead>,
}

type Handle<K> = Rc<State<K>>;

type Nodes<K> = Rc<RefCell<HashMap<K, NodeId>>>;

#[component]
pub fn Tree<K>(
    keys: Prop<Vec<K>>,
    item: Func<K, TreeItem>,
    selected: Prop<Option<K>>,
    #[prop(default = None)] reveal: Prop<Option<K>>,
    #[prop(default = 0.0)] spacing: f32,
    #[prop(default = true)] expand_on_select: Prop<bool>,
    on_select: Callback<K>,
    on_expand: Callback<(K, bool)>,
    on_hover_change: Callback<(K, bool)>,
    #[prop(children)] row: Option<RenderFn<TreeRowHandle<K>>>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
{
    let row = row.expect("tree requires a `row` builder");
    let keys = create_memo(move || keys.get());
    let selected = create_memo(move || selected.get());
    let selection = create_selector(clone!(selected -> move || selected.get()));
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

    let expand_on_select = create_memo(move || expand_on_select.get());
    let state: Handle<K> = Rc::new(State {
        keys: keys.clone(),
        item: item.clone(),
        expand_on_select,
        focus: focus.clone(),
        set_focus,
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

    let nodes: Nodes<K> = Rc::default();
    create_effect(clone!(nodes -> move || {
        let Some(key) = reveal.get() else {
            return;
        };
        let node = nodes.borrow().get(&key).copied();
        if let Some(node) = node {
            with_document(|document| document.reveal_node(node));
        }
    }));

    view! {
        <List spacing>
            <ForEach keys>
                {move |key: K| {
                    let item = create_memo(clone!(item key -> move || item.call(key.clone())));
                    let selected = selection.memo(Some(key.clone()));
                    let hover = on_hover_change.clone();
                    view! {
                        <TreeRow
                            row_key={key}
                            item
                            selected
                            tab_stop={tab_stops.clone()}
                            focused={focused.clone()}
                            state={state.clone()}
                            nodes={nodes.clone()}
                            row={row.clone()}
                            on_hover_change={move |change| hover.call(change)}
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
    tab_stop: Selector<Option<K>>,
    focused: Selector<Option<K>>,
    state: Handle<K>,
    nodes: Nodes<K>,
    row: RenderFn<TreeRowHandle<K>>,
    on_hover_change: Callback<(K, bool)>,
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
    let (blur, click, press, typed) = (state.clone(), state.clone(), state.clone(), state.clone());
    let (blur_key, click_key, press_key, typed_key, content_key) = (
        key.clone(),
        key.clone(),
        key.clone(),
        key.clone(),
        key.clone(),
    );
    let (nodes_key, cleanup_key, toggle_key) = (key.clone(), key.clone(), key.clone());
    let toggling = state.clone();
    let expand: Rc<dyn Fn()> = Rc::new(move || toggle(&toggling, &toggle_key));
    let built = view! {
        <unstyled::Button
            tab_stop={tab_stop.memo(Some(key.clone()))}
            focused={focused.memo(Some(key))}
            accessibility
            on_focus_change={move |has_focus: bool| track_focus(&blur, &blur_key, has_focus)}
            on_click={move || activate(&click, &click_key)}
            on_key={move |event: KeyPress| key_press(&press, &press_key, event)}
            on_text={move |text: String| typeahead(&typed, &typed_key, &text)}
            content={move |button: ButtonHandle| {
                let hovered = button.hovered.clone();
                create_effect(clone!(on_hover_change content_key -> move || {
                    on_hover_change.call((content_key.clone(), hovered.get()));
                }));
                row.call(TreeRowHandle {
                    key: content_key,
                    item,
                    selected,
                    hovered: button.hovered,
                    active: button.active,
                    focused: button.focused,
                    toggle: expand,
                })
            }}
        />
    };
    nodes.borrow_mut().insert(nodes_key, built);
    on_cleanup(move || {
        nodes.borrow_mut().remove(&cleanup_key);
    });
    built
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

fn activate<K>(state: &State<K>, key: &K)
where
    K: Clone + Eq + Hash + 'static,
{
    state.set_focus.set(Some(key.clone()));
    state.on_select.call(key.clone());
    if untrack(|| state.expand_on_select.get()) {
        toggle(state, key);
    }
}

fn toggle<K>(state: &State<K>, key: &K)
where
    K: Clone + Eq + Hash + 'static,
{
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
        select(state, &keys[moved]);
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
        select(state, &keys[matched]);
    }
}

fn select<K>(state: &State<K>, key: &K)
where
    K: Clone + Eq + Hash + 'static,
{
    state.set_focus.set(Some(key.clone()));
    state.on_select.call(key.clone());
}
