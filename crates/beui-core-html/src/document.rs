use be_lib_stable_hash::HashMap;
use std::any::Any;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use beui_html::Node;
use beui_reactive::ScopeContext;
use beui_types::Direction;

use beui_types::node::NodeCell;

use crate::node::NodeId;

#[derive(Default)]
pub(crate) struct Shared {
    style: Vec<(&'static str, String)>,
    placed: Vec<(&'static str, String)>,
    test_ids: Vec<String>,
    described: Option<(u64, accesskit::Node)>,
    /// A description another component gives the node (a tooltip's label), shown where the
    /// node's own accessibility has none.
    description: Option<String>,
    /// The node `set_description` last described for this one (itself or one within it).
    describing: Option<(Node, Weak<RefCell<Shared>>)>,
    aria: Vec<(&'static str, String)>,
}

pub(crate) struct Entry {
    pub(crate) element: Node,
    pub(crate) kind: &'static str,
    pub(crate) shared: Rc<RefCell<Shared>>,
    pub(crate) wraps: u32,
    pub(crate) owner: Option<ScopeContext>,
    pub(crate) direction: Direction,
    /// Whether a list wraps its items, which sticks none of them.
    pub(crate) wraps_items: bool,
    pub(crate) states: Vec<Box<dyn Any>>,
    refs: Vec<Weak<NodeCell>>,
}

#[derive(Default)]
struct Arena {
    slots: Vec<(u32, Option<Entry>)>,
    free: Vec<u32>,
    extensions: HashMap<std::any::TypeId, Rc<dyn Any>>,
    /// Every element whose node has accessibility of its own, by `Node::key`, so
    /// `set_description` finds the first within a subtree by walking only that subtree. It
    /// holds the element, so a key is never reused while its entry stands; `remove_released`
    /// drops an entry once no node holds its `Shared`.
    accessible: HashMap<usize, (Node, Weak<RefCell<Shared>>)>,
}

#[derive(Default)]
struct Removals(RefCell<Vec<NodeId>>);

impl beui_reactive::Release for Removals {
    fn release(&self, key: u64) {
        self.0.borrow_mut().push(NodeId::from_key(key));
    }
}

thread_local! {
    static SCOPE: ScopeContext = {
        let scope = beui_reactive::Scope::root();
        let context = scope.context();
        std::mem::forget(scope);
        context
    };
    static ARENA: RefCell<Arena> = RefCell::default();
    static REMOVALS: Rc<Removals> = Rc::default();
    /// Host nodes (`create_host`) and the child each holds now.
    static HOSTS: RefCell<HashMap<NodeId, Option<NodeId>>> = RefCell::default();
}

/// Makes `host` a host node: a `display: contents` element whose placed styles (what its
/// parent sets to place it: flex, min and max, alignment, grid area) go to the child it holds.
pub(crate) fn make_host(host: NodeId) {
    set_style(host, "display", Some("contents".to_owned()));
    HOSTS.with(|hosts| hosts.borrow_mut().insert(host, None));
}

/// Records the child a host holds now, moving its placed styles from the child it held.
pub(crate) fn set_host_child(host: NodeId, child: Option<NodeId>) {
    let Some(old) = HOSTS.with(|hosts| hosts.borrow_mut().insert(host, child)) else {
        return;
    };
    if old == child {
        return;
    }
    let placed = with_entry(host, |entry| entry.shared.borrow().placed.clone()).unwrap_or_default();
    for (property, value) in placed {
        if let Some(old) = old {
            set_placed_style(old, property, None);
        }
        if let Some(child) = child {
            set_placed_style(child, property, Some(value));
        }
    }
}

/// The node whose box stands for `node`: itself, or for a host node the child it holds
/// (through nested hosts). `None` for a host holding nothing, which has no box.
pub fn boxed(node: NodeId) -> Option<NodeId> {
    match HOSTS.with(|hosts| hosts.borrow().get(&node).copied()) {
        Some(child) => boxed(child?),
        None => Some(node),
    }
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
            let entry = slot.1.take();
            slot.0 = slot.0.wrapping_add(1);
            arena.free.push(node.index());
            if let Some(entry) = entry {
                dropped.push((node, entry));
            }
        }
        dropped
    });
    // A wrapper and its child share one element and one `Shared`, and may go in one batch:
    // only once the whole batch is dropped can the index tell which elements no node holds.
    let keys: Vec<usize> = dropped
        .iter()
        .map(|(_, entry)| entry.element.key())
        .collect();
    for (node, entry) in dropped {
        HOSTS.with(|hosts| hosts.borrow_mut().remove(&node));
        for cell in &entry.refs {
            if let Some(cell) = cell.upgrade()
                && cell.get() == Some(node)
            {
                cell.set(None);
            }
        }
    }
    ARENA.with(|arena| {
        let mut arena = arena.borrow_mut();
        for key in keys {
            if arena
                .accessible
                .get(&key)
                .is_some_and(|(_, shared)| shared.strong_count() == 0)
            {
                arena.accessible.remove(&key);
            }
        }
    });
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
        owner: beui_reactive::current_scope(),
        direction: Direction::Vertical,
        wraps_items: false,
        states: Vec::new(),
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
    if beui_reactive::current_scope().is_some() {
        let removals: Rc<dyn beui_reactive::Release> = REMOVALS.with(|removals| removals.clone());
        beui_reactive::on_release(removals, node.key());
    }
    node
}

pub fn create(kind: &'static str, tag: &'static str) -> NodeId {
    create_on(kind, Node::element(tag), Rc::default(), 0)
}

pub fn adopt(kind: &'static str, element: Node) -> NodeId {
    create_on(kind, element, Rc::default(), 0)
}

/// A node that is its child's own element, so `Interactive` adds no element of its own. A
/// host node (`create_host`) is `display: contents` and has no box, so a node wrapping one
/// gets an element of its own holding the host.
pub fn wrap(kind: &'static str, child: NodeId) -> NodeId {
    if HOSTS.with(|hosts| hosts.borrow().contains_key(&child)) {
        let node = create(kind, "div");
        set_style(node, "display", Some("grid".to_owned()));
        set_style(
            node,
            "grid-template",
            Some("minmax(0, 1fr) / minmax(0, 1fr)".to_owned()),
        );
        append(node, child);
        return node;
    }
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

pub fn wraps(node: NodeId) -> bool {
    with_entry(node, |entry| entry.wraps_items).unwrap_or(false)
}

pub fn set_wraps(node: NodeId, wraps: bool) {
    with_entry(node, |entry| entry.wraps_items = wraps);
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

/// Every style a parent sets to place a child goes through here, so a host node passes it on
/// to the child it holds.
pub fn set_placed_style(node: NodeId, property: &'static str, value: Option<String>) {
    let held = HOSTS.with(|hosts| hosts.borrow().get(&node).copied());
    restyle(node, |entry| {
        write(&mut entry.placed, property, value.clone())
    });
    if let Some(Some(child)) = held {
        set_placed_style(child, property, value);
    }
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
    if id.is_empty() {
        return;
    }
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

#[cfg(target_arch = "wasm32")]
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

pub fn set_accessibility(node: NodeId, accessibility: accesskit::Node, rank: u64) {
    let Some((element, shared)) =
        with_entry(node, |entry| (entry.element.clone(), entry.shared.clone()))
    else {
        return;
    };
    if shared
        .borrow()
        .described
        .as_ref()
        .is_some_and(|(held, _)| *held > rank)
    {
        return;
    }
    ARENA.with(|arena| {
        let mut arena = arena.borrow_mut();
        let held = arena.accessible.get(&element.key());
        if !held.is_some_and(|(_, held)| held.ptr_eq(&Rc::downgrade(&shared))) {
            arena
                .accessible
                .insert(element.key(), (element.clone(), Rc::downgrade(&shared)));
        }
    });
    shared.borrow_mut().described = Some((rank, accessibility));
    apply_aria(&element, &shared);
}

/// Writes the ARIA attributes of a node's accessibility, with the description another gave it.
fn apply_aria(element: &Node, shared: &RefCell<Shared>) {
    let mut shared = shared.borrow_mut();
    let Some((_, accessibility)) = shared.described.as_ref() else {
        return;
    };
    let mut attributes = match &shared.description {
        Some(description)
            if accessibility.description().is_none()
                && accessibility.label() != Some(description.as_str()) =>
        {
            let mut described = accessibility.clone();
            described.set_description(description.clone());
            crate::aria::attributes(&described, element.tag())
        }
        _ => crate::aria::attributes(accessibility, element.tag()),
    };
    if let Some(referenced) = accessibility.active_descendant() {
        let name = format!("beui-{}", referenced.0);
        let target = with_entry(NodeId::from_key(referenced.0), |entry| {
            entry.element.clone()
        });
        if let Some(target) = target {
            target.set_attribute("id", Some(name.clone()));
            attributes.push(("aria-activedescendant", name));
        }
    }
    for (name, _) in &shared.aria {
        if !attributes.iter().any(|(set, _)| set == name) {
            element.set_attribute(name, None);
        }
    }
    for (name, value) in &attributes {
        element.set_attribute(name, Some(value.clone()));
    }
    shared.aria = attributes;
}

/// Describes the first node at or within `node` that has accessibility of its own, in child
/// order, with `description` (`node` itself where none has), where that gives no description
/// and another label, as a tooltip describes the control it wraps; `None` takes it away. The
/// node is found when this is called: one that appears within later is not described.
pub fn set_description(node: NodeId, description: Option<String>) {
    let Some((element, own)) =
        with_entry(node, |entry| (entry.element.clone(), entry.shared.clone()))
    else {
        return;
    };
    let (target_element, target) = ARENA
        .with(|arena| {
            let arena = arena.borrow();
            let found = element.find(&|candidate: &Node| {
                arena
                    .accessible
                    .get(&candidate.key())
                    .is_some_and(|(_, shared)| shared.strong_count() > 0)
            })?;
            let (found, shared) = arena.accessible.get(&found.key())?;
            Some((found.clone(), shared.upgrade()?))
        })
        .unwrap_or((element, own.clone()));
    let previous = own
        .borrow_mut()
        .describing
        .replace((target_element.clone(), Rc::downgrade(&target)))
        .and_then(|(element, previous)| Some((element, previous.upgrade()?)));
    if let Some((element, previous)) =
        previous.filter(|(_, previous)| !Rc::ptr_eq(previous, &target))
    {
        previous.borrow_mut().description = None;
        apply_aria(&element, &previous);
    }
    if target.borrow().description == description {
        return;
    }
    target.borrow_mut().description = description;
    apply_aria(&target_element, &target);
}

/// How many elements the accessible index holds, for tests that it lets released ones go.
#[cfg(test)]
pub(crate) fn accessible_count() -> usize {
    ARENA.with(|arena| arena.borrow().accessible.len())
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
        let mut by_element: HashMap<*const RefCell<Shared>, usize> = HashMap::default();
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
                            accessibility: entry
                                .shared
                                .borrow()
                                .described
                                .as_ref()
                                .map(|(_, described)| described.clone()),
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
        }
        found.into_iter().map(|(about, ..)| about).collect()
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

#[cfg(test)]
mod tests;
