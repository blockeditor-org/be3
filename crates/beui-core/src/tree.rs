use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use beui_tree::reactive::{
    ChildSegment, ChildValue, Children, ComponentContext, Get, IntoChild, IntoProp, IntoSegment,
    IntoSlotHost, Prop, ReadSignal, ScopeContext, Set, Signal, SlotChild, SlotHost, SlotId,
    create_effect, create_signal, current_component, on_cleanup,
};

use crate::base::Sizing;
use crate::base::canvas::CanvasNode;
use crate::base::grid::{GridItem, GridNode};
use crate::base::layers::LayersNode;
use crate::base::list::{ListItem, ListNode};
use crate::base::text::{TextNode, TextPart};
use crate::child_list::{ChildHost, ChildItem};
use crate::current::with_document;
use crate::document::Document;
use crate::node::{NodeId, NodeOf};

#[derive(Default)]
struct NodeBindings {
    target: Cell<Option<NodeId>>,
    states: RefCell<Vec<Box<dyn Any>>>,
    accessibility: RefCell<Option<accesskit::Node>>,
    placed: RefCell<Option<Signal<bool>>>,
    node: RefCell<Option<crate::callback::NodeRef>>,
}

fn bindings() -> Rc<NodeBindings> {
    current_component().extension::<NodeBindings>()
}

pub fn component_node() -> crate::callback::NodeRef {
    let bindings = bindings();
    if let Some(node) = bindings.node.borrow().as_ref() {
        return *node;
    }
    let node = crate::callback::NodeRef::new();
    if let Some(target) = bindings.target.get() {
        node.fill(target);
    }
    bindings.node.replace(Some(node));
    node
}

pub fn component_placed() -> ReadSignal<bool> {
    let bindings = bindings();
    if let Some(target) = bindings.target.get() {
        let measured = with_document(|document| document.watch_measure(target));
        let placed = create_signal(measured.get_untracked().is_some());
        create_effect(move || placed.set(measured.get().is_some()));
        return placed.read_only();
    }
    if let Some(placed) = bindings.placed.borrow().as_ref() {
        return placed.read_only();
    }
    let placed = create_signal(false);
    bindings.placed.replace(Some(placed));
    placed.read_only()
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
    fn finish_component(&mut self, component: &ComponentContext, scope: &ScopeContext) {
        let root = *self;
        let Some(bindings) = component.find_extension::<NodeBindings>() else {
            with_document(|document| document.name_component(root, component.record()));
            return;
        };
        bindings.target.set(Some(root));
        let states = bindings.states.take();
        let accessibility = bindings.accessibility.take();
        let placed = bindings.placed.take();
        if let Some(node) = bindings.node.borrow().as_ref() {
            node.fill(root);
        }
        if let Some(placed) = placed {
            let watched = with_document(|document| document.watch_measure(root));
            scope.run(|| create_effect(move || placed.set(watched.get().is_some())));
        }
        with_document(|document| {
            document.name_component(root, component.record());
            for state in states {
                document.set_component_state_dyn(root, state);
            }
            if let Some(node) = accessibility {
                document.set_accessibility(root, node);
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

impl SlotChild for NodeId {
    type Stored = NodeId;

    fn store(self) -> NodeId {
        self
    }
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

#[diagnostic::on_unimplemented(
    message = "a `{Self}` does not keep placed children",
    label = "implement `ParentKind` for the node that keeps these children"
)]
pub trait ParentKind: ChildHost<Stored: Clone + 'static> + Sized {
    type Data: Clone + 'static;

    fn item(node: NodeId, data: Self::Data) -> Self::Stored;

    fn update(_document: &mut Document, _parent: NodeOf<Self>, _node: NodeId, _data: Self::Data) {}
}

pub trait BareChild: ParentKind {
    fn bare() -> Self::Data;
}

pub struct Placed<P: ParentKind> {
    pub node: NodeId,
    pub data: Prop<P::Data>,
}

pub type ListChild = Placed<ListNode>;

impl<P: ParentKind> Placed<P> {
    pub fn new(node: NodeId, data: impl IntoProp<P::Data>) -> Self {
        Self {
            node,
            data: data.into_prop(),
        }
    }

    fn store_in(self, parent: NodeId, latest: &Latest<P>) -> P::Stored {
        let Placed { node, data } = self;
        let initial = data.get_untracked();
        if let Prop::Dynamic(read) = data {
            let held = Rc::clone(latest);
            create_effect(move || {
                let data = read.get();
                held.borrow_mut().insert(node, data.clone());
                with_document(|document| {
                    if let Some(parent) = document.arena.kind_of::<P>(parent) {
                        P::update(document, parent, node, data);
                    }
                });
            });
            let held = Rc::clone(latest);
            on_cleanup(move || {
                held.borrow_mut().remove(&node);
            });
        }
        P::item(node, initial)
    }
}

impl<P: ParentKind> BuildsNode for Placed<P> {
    fn built_node(&self) -> NodeId {
        self.node
    }
}

impl<P: ParentKind> ChildValue for Placed<P> {
    fn finish_component(&mut self, component: &ComponentContext, scope: &ScopeContext) {
        self.node.finish_component(component, scope);
    }
}

impl<P: ParentKind> SlotChild for Placed<P> {
    type Stored = P::Stored;

    fn store(self) -> P::Stored {
        P::item(self.node, self.data.get_untracked())
    }
}

type Latest<P> = Rc<RefCell<HashMap<NodeId, <P as ParentKind>::Data>>>;

struct NodeHost<P: ParentKind> {
    parent: NodeOf<P>,
    latest: Latest<P>,
}

impl<P: ParentKind> NodeHost<P> {
    fn current(&self, item: P::Stored) -> P::Stored {
        let Some(node) = item.node() else {
            return item;
        };
        match self.latest.borrow().get(&node) {
            Some(data) => P::item(node, data.clone()),
            None => item,
        }
    }
}

impl<P: ParentKind> SlotHost<Placed<P>> for NodeHost<P> {
    fn store(&self, child: Placed<P>) -> P::Stored {
        child.store_in(self.parent.id(), &self.latest)
    }

    fn append(&self, stored: P::Stored) {
        let stored = self.current(stored);
        with_document(|document| document.append_child_item(self.parent, stored));
    }

    fn open(&self) -> SlotId {
        with_document(|document| document.open_child_slot(self.parent))
    }

    fn fill(&self, slot: SlotId, items: Vec<P::Stored>) {
        let items = items.into_iter().map(|item| self.current(item)).collect();
        with_document(|document| {
            if document.contains(self.parent) {
                document.fill_child_slot(self.parent, slot, items);
            }
        });
    }
}

impl<P: ParentKind> IntoSlotHost<Placed<P>> for NodeOf<P> {
    fn into_slot_host(self) -> Rc<dyn SlotHost<Placed<P>>> {
        Rc::new(NodeHost {
            parent: self,
            latest: Rc::default(),
        })
    }
}

#[diagnostic::on_unimplemented(
    message = "a child made for a `{P}` cannot be a child of this component",
    label = "`@sizing` makes a list child: remove it, or put this child in a `Row`, `Column`, `List` or `Stack`"
)]
pub trait FromPlaced<P: ParentKind> {
    fn from_placed(child: Placed<P>) -> Self;
}

impl<P: ParentKind> FromPlaced<P> for Placed<P> {
    fn from_placed(child: Placed<P>) -> Self {
        child
    }
}

impl<P: ParentKind, T: FromPlaced<P>> IntoChild<T> for Placed<P> {
    fn into_child(self) -> T {
        T::from_placed(self)
    }
}

impl<P: ParentKind, T: FromPlaced<P> + SlotChild> IntoSegment<T> for Placed<P> {
    fn into_segment(self) -> ChildSegment<T> {
        ChildSegment::One(self.into_child())
    }
}

impl<P: ParentKind, T: FromPlaced<P> + SlotChild> From<Placed<P>> for Children<T> {
    fn from(child: Placed<P>) -> Self {
        Self::from(ChildSegment::One(child.into_child()))
    }
}

impl<P: ParentKind> IntoSegment<Placed<P>> for Option<Placed<P>> {
    fn into_segment(self) -> ChildSegment<Placed<P>> {
        ChildSegment::Many(self.into_iter().collect())
    }
}

impl<P: BareChild> IntoChild<Placed<P>> for NodeId {
    fn into_child(self) -> Placed<P> {
        Placed {
            node: self,
            data: Prop::Static(P::bare()),
        }
    }
}

impl<P: BareChild> IntoSegment<Placed<P>> for NodeId {
    fn into_segment(self) -> ChildSegment<Placed<P>> {
        ChildSegment::One(self.into_child())
    }
}

impl<P: BareChild> IntoSegment<Placed<P>> for Option<NodeId> {
    fn into_segment(self) -> ChildSegment<Placed<P>> {
        ChildSegment::Many(self.into_iter().map(IntoChild::into_child).collect())
    }
}

impl<P: BareChild> From<NodeId> for Children<Placed<P>> {
    fn from(node: NodeId) -> Self {
        Self::from(IntoSegment::<Placed<P>>::into_segment(node))
    }
}

impl ParentKind for ListNode {
    type Data = Sizing;

    fn item(node: NodeId, size: Sizing) -> ListItem {
        ListItem { child: node, size }
    }

    fn update(document: &mut Document, list: NodeOf<Self>, node: NodeId, size: Sizing) {
        document.set_child_size(list, node, size);
    }
}

impl BareChild for ListNode {
    fn bare() -> Sizing {
        Sizing::default()
    }
}

impl ParentKind for GridNode {
    type Data = usize;

    fn item(node: NodeId, span: usize) -> GridItem {
        GridItem { child: node, span }
    }

    fn update(document: &mut Document, grid: NodeOf<Self>, node: NodeId, span: usize) {
        document.set_grid_span(grid, node, span);
    }
}

impl BareChild for GridNode {
    fn bare() -> usize {
        1
    }
}

impl ParentKind for LayersNode {
    type Data = ();

    fn item(node: NodeId, _: ()) -> NodeId {
        node
    }
}

impl BareChild for LayersNode {
    fn bare() {}
}

impl ParentKind for CanvasNode {
    type Data = ();

    fn item(node: NodeId, _: ()) -> NodeId {
        node
    }
}

impl ParentKind for TextNode {
    type Data = bool;

    fn item(node: NodeId, anchored: bool) -> TextPart {
        match anchored {
            true => TextPart::Anchored(node),
            false => TextPart::Inline(node),
        }
    }
}

#[diagnostic::on_unimplemented(
    message = "`@sizing` cannot size a `{Self}`",
    label = "`@sizing` takes a node, or an `Option` of one"
)]
pub trait WithSizing {
    type Sized;

    fn with_sizing(self, size: Prop<Sizing>) -> Self::Sized;
}

impl WithSizing for NodeId {
    type Sized = ListChild;

    fn with_sizing(self, size: Prop<Sizing>) -> ListChild {
        ListChild::new(self, size)
    }
}

impl WithSizing for Option<NodeId> {
    type Sized = Option<ListChild>;

    fn with_sizing(self, size: Prop<Sizing>) -> Option<ListChild> {
        self.map(|node| ListChild::new(node, size))
    }
}

pub fn with_sizing<C: WithSizing>(child: C, size: impl IntoProp<Sizing>) -> C::Sized {
    child.with_sizing(size.into_prop())
}
