use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui_tree::reactive::{
    ChildSegment, ChildValue, Children, ComponentContext, IntoChild, IntoProp, IntoSegment,
    IntoSlotHost, Memo, Prop, ReadSignal, Scope, SlotChild, SlotHost, SlotId, WriteSignal,
    create_effect, create_signal, current_component,
};

use crate::base::child_list::ChildHost;
use crate::base::list::{ListItem, ListNode};
use crate::base::offset::OffsetNode;
use crate::base::{ItemSize, Sizing};
use crate::current::{try_with_document, with_document};
use crate::geometry::{Rect, Vec2};
use crate::node::{NodeId, NodeOf};

#[derive(Default)]
struct NodeBindings {
    target: Cell<Option<NodeId>>,
    states: RefCell<Vec<Box<dyn Any>>>,
    accessibility: RefCell<Option<accesskit::Node>>,
    size: RefCell<Option<(ReadSignal<Vec2>, WriteSignal<Vec2>)>>,
    placement: RefCell<Option<(ReadSignal<Rect>, WriteSignal<Rect>)>>,
    placed: RefCell<Option<(ReadSignal<bool>, WriteSignal<bool>)>>,
}

fn bindings() -> Rc<NodeBindings> {
    current_component().extension::<NodeBindings>()
}

pub fn component_size() -> ReadSignal<Vec2> {
    let bindings = bindings();
    if let Some(target) = bindings.target.get() {
        return with_document(|document| document.watch_size(target));
    }
    if let Some((read, _)) = bindings.size.borrow().as_ref() {
        return read.clone();
    }
    let (read, write) = create_signal(Vec2::ZERO);
    bindings.size.replace(Some((read.clone(), write)));
    read
}

pub fn component_placed() -> ReadSignal<bool> {
    let bindings = bindings();
    if let Some(target) = bindings.target.get() {
        return with_document(|document| document.watch_placed(target));
    }
    if let Some((read, _)) = bindings.placed.borrow().as_ref() {
        return read.clone();
    }
    let (read, write) = create_signal(false);
    bindings.placed.replace(Some((read.clone(), write)));
    read
}

pub fn component_rect() -> ReadSignal<Rect> {
    let bindings = bindings();
    if let Some(target) = bindings.target.get() {
        return with_document(|document| document.watch_placement(target));
    }
    if let Some((read, _)) = bindings.placement.borrow().as_ref() {
        return read.clone();
    }
    let (read, write) = create_signal(Rect::ZERO);
    bindings.placement.replace(Some((read.clone(), write)));
    read
}

pub fn set_component_state<T: 'static>(state: T) {
    let bindings = bindings();
    match bindings.target.get() {
        Some(target) => {
            with_document(|document| document.set_component_state_dyn(target, Box::new(state)))
        }
        None => bindings.states.borrow_mut().push(Box::new(state)),
    }
}

pub fn component_accessibility(node: impl IntoProp<accesskit::Node>) {
    let bindings = bindings();
    let node = node.into_prop();
    create_effect(move || {
        let node = node.get();
        match bindings.target.get() {
            Some(target) => with_document(|document| document.set_accessibility(target, node)),
            None => {
                bindings.accessibility.replace(Some(node));
            }
        }
    });
}

impl ChildValue for NodeId {
    fn adopt_scope(&mut self, scope: Scope) {
        let node = *self;
        with_document(|document| document.register_node_scope(node, scope));
    }

    fn finish_component(&mut self, component: &ComponentContext, scope: &Scope) {
        let root = *self;
        let bindings = component.extension::<NodeBindings>();
        bindings.target.set(Some(root));
        let states = bindings.states.take();
        let accessibility = bindings.accessibility.take();
        let size = bindings.size.take();
        let placement = bindings.placement.take();
        let placed = bindings.placed.take();
        if let Some((_, write)) = placed {
            let watched = with_document(|document| document.watch_placed(root));
            scope.run(|| create_effect(move || write.set(watched.get())));
        }
        with_document(|document| {
            document.name_component(root, component.name());
            for state in states {
                document.set_component_state_dyn(root, state);
            }
            if let Some(node) = accessibility {
                document.set_accessibility(root, node);
            }
            if let Some((read, write)) = size {
                document.register_size_watcher(root, read, write);
            }
            if let Some((read, write)) = placement {
                document.register_placement_watcher(root, read, write);
            }
        });
    }
}

#[diagnostic::on_unimplemented(
    message = "a `{Self}` is no node, so `@test_id` and `@node_ref` have nothing to name",
    label = "put `@test_id` or `@node_ref` on a tag that builds a node, inside this one or around it"
)]
pub trait BuildsNode {
    fn built_node(&self) -> NodeId;
}

impl BuildsNode for NodeId {
    fn built_node(&self) -> NodeId {
        *self
    }
}

#[diagnostic::on_unimplemented(
    message = "a run of `{Self}` children is not kept by a node",
    label = "this parent keeps its children in a node, and `{Self}` is no node"
)]
pub trait NodeSlot: SlotChild {
    type Host: ChildHost<Stored = Self::Stored>;

    fn store_in(self, _parent: NodeId) -> Self::Stored {
        self.store()
    }
}

struct NodeHost<H>(NodeOf<H>);

impl<C: NodeSlot> SlotHost<C> for NodeHost<C::Host> {
    fn store(&self, child: C) -> C::Stored {
        child.store_in(self.0.id())
    }

    fn append(&self, stored: C::Stored) {
        with_document(|document| document.append_child_item(self.0, stored));
    }

    fn open(&self) -> SlotId {
        with_document(|document| document.open_child_slot(self.0))
    }

    fn fill(&self, slot: SlotId, items: Vec<C::Stored>) {
        with_document(|document| document.fill_child_slot(self.0, slot, items));
    }

    fn adopt_scope(&self, scope: Scope) {
        let parent = self.0.id();
        with_document(|document| document.register_node_scope(parent, scope));
    }
}

impl<C: NodeSlot> IntoSlotHost<C> for NodeOf<C::Host> {
    fn into_slot_host(self) -> Rc<dyn SlotHost<C>> {
        Rc::new(NodeHost(self))
    }
}

pub fn remove_stored_node(node: NodeId) {
    try_with_document(|document| document.remove_node(node));
}

impl SlotChild for NodeId {
    type Stored = NodeId;

    fn store(self) -> NodeId {
        self
    }

    fn discard(stored: &NodeId) {
        remove_stored_node(*stored);
    }
}

impl NodeSlot for NodeId {
    type Host = OffsetNode;
}

impl IntoChild<NodeId> for NodeId {
    fn into_child(self) -> NodeId {
        self
    }
}

impl IntoSegment<NodeId> for NodeId {
    fn into_segment(self) -> ChildSegment<NodeId> {
        ChildSegment::One(self)
    }
}

impl From<NodeId> for Children<NodeId> {
    fn from(node: NodeId) -> Self {
        Self::from(ChildSegment::One(node))
    }
}

pub struct ListChild {
    pub node: NodeId,
    pub size: Prop<Sizing>,
}

impl ListChild {
    pub fn new(node: NodeId, size: impl IntoProp<Sizing>) -> Self {
        Self {
            node,
            size: size.into_prop(),
        }
    }
}

impl BuildsNode for ListChild {
    fn built_node(&self) -> NodeId {
        self.node
    }
}

impl ChildValue for ListChild {
    fn adopt_scope(&mut self, scope: Scope) {
        self.node.adopt_scope(scope);
    }

    fn finish_component(&mut self, component: &ComponentContext, scope: &Scope) {
        self.node.finish_component(component, scope);
    }
}

impl SlotChild for ListChild {
    type Stored = ListItem;

    fn store(self) -> ListItem {
        ListItem {
            child: self.node,
            size: self.size.peek(),
        }
    }

    fn discard(stored: &ListItem) {
        remove_stored_node(stored.child);
    }
}

impl NodeSlot for ListChild {
    type Host = ListNode;

    fn store_in(self, parent: NodeId) -> ListItem {
        let initial = self.size.peek();
        let ListChild { node, size } = self;
        if let Prop::Dynamic(read) = size {
            create_effect(move || {
                let size = read();
                with_document(|document| {
                    if let Some(list) = document.arena.kind_of::<ListNode>(parent) {
                        document.set_child_size(list, node, size);
                    }
                });
            });
        }
        ListItem {
            child: node,
            size: initial,
        }
    }
}

impl IntoChild<ListChild> for NodeId {
    fn into_child(self) -> ListChild {
        ListChild::new(self, Sizing::default())
    }
}

#[diagnostic::on_unimplemented(
    message = "`@sizing` only applies to a child of a list",
    label = "remove `@sizing`, or put this child in a `Row`, `Column`, `List` or `Stack`"
)]
pub trait AcceptsSizing {
    fn from_list_child(child: ListChild) -> Self;
}

impl AcceptsSizing for ListChild {
    fn from_list_child(child: ListChild) -> Self {
        child
    }
}

impl<T: AcceptsSizing> IntoChild<T> for ListChild {
    fn into_child(self) -> T {
        T::from_list_child(self)
    }
}

impl IntoSegment<ListChild> for NodeId {
    fn into_segment(self) -> ChildSegment<ListChild> {
        ChildSegment::One(self.into_child())
    }
}

impl<T: AcceptsSizing + SlotChild> IntoSegment<T> for ListChild {
    fn into_segment(self) -> ChildSegment<T> {
        ChildSegment::One(self.into_child())
    }
}

impl From<NodeId> for Children<ListChild> {
    fn from(node: NodeId) -> Self {
        Self::from(IntoSegment::<ListChild>::into_segment(node))
    }
}

impl<T: AcceptsSizing + SlotChild> From<ListChild> for Children<T> {
    fn from(child: ListChild) -> Self {
        Self::from(child.into_segment())
    }
}

impl IntoProp<Sizing> for ItemSize {
    fn into_prop(self) -> Prop<Sizing> {
        Prop::Static(self.into())
    }
}

impl IntoProp<Sizing> for Prop<ItemSize> {
    fn into_prop(self) -> Prop<Sizing> {
        self.map(Sizing::from)
    }
}

impl IntoProp<Sizing> for ReadSignal<ItemSize> {
    fn into_prop(self) -> Prop<Sizing> {
        Prop::Dynamic(Rc::new(move || self.get().into()))
    }
}

impl IntoProp<Sizing> for Memo<ItemSize> {
    fn into_prop(self) -> Prop<Sizing> {
        Prop::Dynamic(Rc::new(move || self.get().into()))
    }
}
