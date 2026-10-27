use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use beui_html::Node;
use beui_types::Direction;
use reactive::ScopeContext;

use beui_types::node::NodeCell;

use crate::node::NodeId;

#[derive(Default)]
pub(crate) struct Shared {
    style: Vec<(&'static str, String)>,
    placed: Vec<(&'static str, String)>,
    test_ids: Vec<String>,
}

pub(crate) struct Entry {
    pub(crate) element: Node,
    pub(crate) kind: &'static str,
    pub(crate) shared: Rc<RefCell<Shared>>,
    pub(crate) wraps: u32,
    pub(crate) owner: Option<ScopeContext>,
    pub(crate) direction: Direction,
    pub(crate) states: Vec<Box<dyn Any>>,
    pub(crate) accessibility: Option<accesskit::Node>,
    refs: Vec<Weak<NodeCell>>,
}

#[derive(Default)]
struct Arena {
    slots: Vec<(u32, Option<Entry>)>,
    free: Vec<u32>,
    extensions: HashMap<std::any::TypeId, Rc<dyn Any>>,
}

#[derive(Default)]
struct Removals(RefCell<Vec<NodeId>>);

impl reactive::Release for Removals {
    fn release(&self, key: u64) {
        self.0.borrow_mut().push(NodeId::from_key(key));
    }
}

thread_local! {
    static SCOPE: ScopeContext = {
        let scope = reactive::Scope::root();
        let context = scope.context();
        std::mem::forget(scope);
        context
    };
    static ARENA: RefCell<Arena> = RefCell::default();
    static REMOVALS: Rc<Removals> = Rc::default();
}

fn take_removals() -> Vec<NodeId> {
    REMOVALS.with(|removals| std::mem::take(&mut *removals.0.borrow_mut()))
}

fn remove_released() {
    let removed = take_removals();
    if removed.is_empty() {
        return;
    }
    let dropped: Vec<(NodeId, Entry)> = ARENA.with(|arena| {
        let mut arena = arena.borrow_mut();
        let mut dropped = Vec::new();
        for node in removed {
            let Some(slot) = arena.slots.get_mut(node.index() as usize) else {
                continue;
            };
            if slot.0 != node.generation() {
                continue;
            }
            if let Some(entry) = slot.1.take() {
                dropped.push((node, entry));
            }
            slot.0 = slot.0.wrapping_add(1);
            arena.free.push(node.index());
        }
        dropped
    });
    for (node, entry) in dropped {
        for cell in &entry.refs {
            if let Some(cell) = cell.upgrade()
                && cell.get() == Some(node)
            {
                cell.set(None);
            }
        }
    }
}

pub(crate) fn fill_ref(node: NodeId, cell: Weak<NodeCell>) {
    with_entry(node, |entry| {
        entry.refs.retain(|cell| cell.strong_count() > 0);
        entry.refs.push(cell);
    });
}

fn create_on(kind: &'static str, element: Node, shared: Rc<RefCell<Shared>>, wraps: u32) -> NodeId {
    crate::tree::install_hooks();
    remove_released();
    let entry = Entry {
        element,
        kind,
        shared,
        wraps,
        owner: reactive::current_scope(),
        direction: Direction::Vertical,
        states: Vec::new(),
        accessibility: None,
        refs: Vec::new(),
    };
    let node = ARENA.with(|arena| {
        let mut arena = arena.borrow_mut();
        match arena.free.pop() {
            Some(index) => {
                let slot = &mut arena.slots[index as usize];
                slot.1 = Some(entry);
                NodeId::new(index, slot.0)
            }
            None => {
                let index = u32::try_from(arena.slots.len()).expect("fewer than 2^32 nodes");
                arena.slots.push((0, Some(entry)));
                NodeId::new(index, 0)
            }
        }
    });
    if reactive::current_scope().is_some() {
        let removals: Rc<dyn reactive::Release> = REMOVALS.with(|removals| removals.clone());
        reactive::on_release(removals, node.key());
    }
    node
}

pub fn create(kind: &'static str, tag: &'static str) -> NodeId {
    create_on(kind, Node::element(tag), Rc::default(), 0)
}

pub fn wrap(kind: &'static str, child: NodeId) -> NodeId {
    let (element, shared, wraps) = with_entry(child, |entry| {
        (entry.element.clone(), entry.shared.clone(), entry.wraps + 1)
    })
    .expect("a wrapped child is built first");
    #[cfg(target_arch = "wasm32")]
    crate::dom::forget_known();
    create_on(kind, element, shared, wraps)
}

pub(crate) fn with_entry<R>(node: NodeId, read: impl FnOnce(&mut Entry) -> R) -> Option<R> {
    ARENA.with(|arena| {
        let mut arena = arena.borrow_mut();
        let slot = arena.slots.get_mut(node.index() as usize)?;
        if slot.0 != node.generation() {
            return None;
        }
        slot.1.as_mut().map(read)
    })
}

pub fn contains(node: NodeId) -> bool {
    with_entry(node, |_| ()).is_some()
}

pub fn element(node: NodeId) -> Option<Node> {
    with_entry(node, |entry| entry.element.clone())
}

pub fn kind(node: NodeId) -> Option<&'static str> {
    with_entry(node, |entry| entry.kind)
}

pub fn owner(node: NodeId) -> Option<ScopeContext> {
    with_entry(node, |entry| entry.owner.clone()).flatten()
}

pub fn direction(node: NodeId) -> Direction {
    with_entry(node, |entry| entry.direction).unwrap_or(Direction::Vertical)
}

pub fn set_direction(node: NodeId, direction: Direction) {
    with_entry(node, |entry| entry.direction = direction);
}

fn write(
    list: &mut Vec<(&'static str, String)>,
    property: &'static str,
    value: Option<String>,
) -> bool {
    let held = list.iter().position(|(name, _)| *name == property);
    match (held, value) {
        (Some(index), Some(value)) if list[index].1 == value => false,
        (Some(index), Some(value)) => {
            list[index].1 = value;
            true
        }
        (Some(index), None) => {
            list.remove(index);
            true
        }
        (None, Some(value)) => {
            list.push((property, value));
            true
        }
        (None, None) => false,
    }
}

fn restyle(node: NodeId, change: impl FnOnce(&mut Shared) -> bool) {
    let written = with_entry(node, |entry| {
        let mut shared = entry.shared.borrow_mut();
        change(&mut shared).then(|| {
            (
                entry.element.clone(),
                serialize(&shared.style, &shared.placed),
            )
        })
    })
    .flatten();
    if let Some((element, style)) = written {
        element.set_attribute("style", (!style.is_empty()).then_some(style));
    }
}

pub fn set_style(node: NodeId, property: &'static str, value: Option<String>) {
    restyle(node, |entry| write(&mut entry.style, property, value));
}

pub fn set_placed_style(node: NodeId, property: &'static str, value: Option<String>) {
    restyle(node, |entry| write(&mut entry.placed, property, value));
}

fn serialize(style: &[(&'static str, String)], placed: &[(&'static str, String)]) -> String {
    let mut out = String::new();
    let own = style
        .iter()
        .filter(|(name, _)| !placed.iter().any(|(held, _)| held == name));
    for (name, value) in own.chain(placed) {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(name);
        out.push_str(": ");
        out.push_str(value);
        out.push(';');
    }
    out
}

pub fn set_attribute(node: NodeId, name: &str, value: Option<String>) {
    if let Some(element) = element(node) {
        element.set_attribute(name, value);
    }
}

pub fn set_markup(node: NodeId, markup: Option<String>) {
    if let Some(element) = element(node) {
        element.set_markup(markup);
    }
}

pub fn append(parent: NodeId, child: NodeId) {
    let (Some(parent), Some(child)) = (element(parent), element(child)) else {
        return;
    };
    let host = beui_tree::reactive::IntoSlotHost::<Node>::into_slot_host(&parent);
    host.append(child);
}

pub fn add_test_id(node: NodeId, id: String) {
    let ids = with_entry(node, |entry| {
        let mut shared = entry.shared.borrow_mut();
        if !shared.test_ids.contains(&id) {
            shared.test_ids.push(id);
        }
        shared.test_ids.join(" ")
    });
    if let Some(ids) = ids {
        set_attribute(node, "data-testid", Some(ids));
    }
}

pub fn remove_test_id(node: NodeId, id: &str) {
    let ids = with_entry(node, |entry| {
        let mut shared = entry.shared.borrow_mut();
        shared.test_ids.retain(|held| held != id);
        shared.test_ids.join(" ")
    });
    if let Some(ids) = ids {
        set_attribute(node, "data-testid", (!ids.is_empty()).then_some(ids));
    }
}

fn outermost(keep: impl Fn(&Entry) -> bool) -> Vec<(NodeId, Node)> {
    ARENA.with(|arena| {
        let arena = arena.borrow();
        let mut found: Vec<(NodeId, Node, Rc<RefCell<Shared>>, u32)> = Vec::new();
        for (index, (generation, entry)) in arena.slots.iter().enumerate() {
            let Some(entry) = entry.as_ref().filter(|entry| keep(entry)) else {
                continue;
            };
            let node = NodeId::new(index as u32, *generation);
            match found
                .iter_mut()
                .find(|held| Rc::ptr_eq(&held.2, &entry.shared))
            {
                Some(held) if held.3 < entry.wraps => {
                    held.0 = node;
                    held.3 = entry.wraps;
                }
                Some(_) => {}
                None => found.push((
                    node,
                    entry.element.clone(),
                    entry.shared.clone(),
                    entry.wraps,
                )),
            }
        }
        found
            .into_iter()
            .map(|(node, element, _, _)| (node, element))
            .collect()
    })
}

pub fn nodes_with_test_id(id: &str) -> Vec<NodeId> {
    outermost(|entry| entry.shared.borrow().test_ids.iter().any(|held| held == id))
        .into_iter()
        .map(|(node, _)| node)
        .collect()
}

pub fn set_state(node: NodeId, state: Box<dyn Any>) {
    with_entry(node, |entry| entry.states.push(state));
}

pub fn state<T: Clone + 'static>(node: NodeId) -> Option<T> {
    with_entry(node, |entry| {
        entry
            .states
            .iter()
            .rev()
            .find_map(|state| state.downcast_ref::<T>())
            .cloned()
    })
    .flatten()
}

pub fn set_accessibility(node: NodeId, accessibility: accesskit::Node) {
    with_entry(node, |entry| entry.accessibility = Some(accessibility));
}

pub fn in_document_scope<T>(run: impl FnOnce() -> T) -> T {
    let scope = SCOPE.with(ScopeContext::clone);
    scope.run(run)
}

pub fn extension_or_insert_with<T: Clone + 'static>(make: impl FnOnce() -> T) -> T {
    let id = std::any::TypeId::of::<T>();
    let held = ARENA.with(|arena| arena.borrow().extensions.get(&id).cloned());
    if let Some(held) = held {
        return held
            .downcast_ref::<T>()
            .expect("extensions are keyed by their type")
            .clone();
    }
    let made = in_document_scope(make);
    let held = ARENA.with(|arena| {
        arena
            .borrow_mut()
            .extensions
            .entry(id)
            .or_insert_with(|| Rc::new(made))
            .clone()
    });
    held.downcast_ref::<T>()
        .expect("extensions are keyed by their type")
        .clone()
}

pub struct About {
    pub node: NodeId,
    pub element: Node,
    pub kinds: Vec<&'static str>,
    pub test_ids: Vec<String>,
    pub accessibility: Option<accesskit::Node>,
}

pub fn about_elements() -> Vec<About> {
    ARENA.with(|arena| {
        let arena = arena.borrow();
        let mut found: Vec<(About, u32)> = Vec::new();
        let mut by_element: HashMap<*const RefCell<Shared>, usize> = HashMap::new();
        for (index, (generation, entry)) in arena.slots.iter().enumerate() {
            let Some(entry) = entry.as_ref() else {
                continue;
            };
            let node = NodeId::new(index as u32, *generation);
            let at = *by_element
                .entry(Rc::as_ptr(&entry.shared))
                .or_insert_with(|| {
                    found.push((
                        About {
                            node,
                            element: entry.element.clone(),
                            kinds: Vec::new(),
                            test_ids: entry.shared.borrow().test_ids.clone(),
                            accessibility: None,
                        },
                        entry.wraps,
                    ));
                    found.len() - 1
                });
            let (about, wraps) = &mut found[at];
            if *wraps < entry.wraps {
                about.node = node;
                *wraps = entry.wraps;
            }
            about.kinds.push(entry.kind);
            if about.accessibility.is_none() {
                about.accessibility = entry.accessibility.clone();
            }
        }
        found.into_iter().map(|(about, _)| about).collect()
    })
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn nodes() -> Vec<(NodeId, Node)> {
    outermost(|_| true)
}

pub fn style_value(node: NodeId, property: &str) -> Option<String> {
    with_entry(node, |entry| {
        let shared = entry.shared.borrow();
        shared
            .placed
            .iter()
            .chain(&shared.style)
            .find(|(name, _)| *name == property)
            .map(|(_, value)| value.clone())
    })
    .flatten()
}
