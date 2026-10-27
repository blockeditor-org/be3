use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use std::rc::Weak;

use beui_tree::reactive::{
    ComponentContext, Get, IntoProp, IntoSlotHost, Prop, ReadSignal, ScopeContext, Set, Signal,
    SlotHost, SlotId, create_effect, create_signal, current_component, on_cleanup,
};
use beui_types::node::{CoreHooks, NodeCell, install_core_hooks};

use crate::base::Sizing;
use crate::base::canvas::CanvasNode;
use crate::base::grid::{GridItem, GridNode};
use crate::base::layers::LayersNode;
use crate::base::list::{ListItem, ListNode};
use crate::base::text::{TextNode, TextPart};
use crate::child_list::ChildHost;
use crate::current::with_document;
use crate::document::Document;
use crate::node::{NodeId, NodeOf};

pub use beui_types::node::BuildsNode;

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

fn finish_component(root: NodeId, component: &ComponentContext, scope: &ScopeContext) {
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

fn fill_ref(node: NodeId, cell: Weak<NodeCell>) {
    crate::current::try_with_document(|document| document.register_node_ref(node, cell));
}

pub(crate) fn install_hooks() {
    install_core_hooks(CoreHooks {
        fill_ref,
        finish_component,
    });
}

#[diagnostic::on_unimplemented(
    message = "a `{Self}` does not keep placed children",
    label = "implement `ParentKind` for the node that keeps these children"
)]
pub trait ParentKind: ChildHost<Stored: Clone + 'static> + Sized {
    type Kind: beui_types::tree::ParentKind;

    fn item(node: NodeId, data: Data<Self>) -> Self::Stored;

    fn update(_document: &mut Document, _parent: NodeOf<Self>, _node: NodeId, _data: Data<Self>) {}
}

pub type Data<P> = <<P as ParentKind>::Kind as beui_types::tree::ParentKind>::Data;

pub type ListChild = Placed<ListKind>;

fn store_in<P: ParentKind>(
    child: Placed<P::Kind>,
    parent: NodeId,
    latest: &Latest<P>,
) -> PlacedItem<P::Kind> {
    let Placed { node, data } = child;
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
    PlacedItem {
        node,
        data: initial,
    }
}

type Latest<P> = Rc<RefCell<HashMap<NodeId, Data<P>>>>;

struct NodeHost<P: ParentKind> {
    parent: NodeOf<P>,
    latest: Latest<P>,
}

impl<P: ParentKind> NodeHost<P> {
    fn current(&self, item: PlacedItem<P::Kind>) -> P::Stored {
        match self.latest.borrow().get(&item.node) {
            Some(data) => P::item(item.node, data.clone()),
            None => P::item(item.node, item.data),
        }
    }
}

impl<P: ParentKind> SlotHost<Placed<P::Kind>> for NodeHost<P> {
    fn store(&self, child: Placed<P::Kind>) -> PlacedItem<P::Kind> {
        store_in::<P>(child, self.parent.id(), &self.latest)
    }

    fn append(&self, stored: PlacedItem<P::Kind>) {
        let stored = self.current(stored);
        with_document(|document| document.append_child_item(self.parent, stored));
    }

    fn open(&self) -> SlotId {
        with_document(|document| document.open_child_slot(self.parent))
    }

    fn fill(&self, slot: SlotId, items: Vec<PlacedItem<P::Kind>>) {
        let items = items.into_iter().map(|item| self.current(item)).collect();
        with_document(|document| {
            if document.contains(self.parent) {
                document.fill_child_slot(self.parent, slot, items);
            }
        });
    }
}

impl<P: ParentKind> IntoSlotHost<Placed<P::Kind>> for NodeOf<P> {
    fn into_slot_host(self) -> Rc<dyn SlotHost<Placed<P::Kind>>> {
        Rc::new(NodeHost {
            parent: self,
            latest: Rc::default(),
        })
    }
}

impl ParentKind for ListNode {
    type Kind = ListKind;

    fn item(node: NodeId, size: Sizing) -> ListItem {
        ListItem { child: node, size }
    }

    fn update(document: &mut Document, list: NodeOf<Self>, node: NodeId, size: Sizing) {
        document.set_child_size(list, node, size);
    }
}

impl ParentKind for GridNode {
    type Kind = GridKind;

    fn item(node: NodeId, span: usize) -> GridItem {
        GridItem { child: node, span }
    }

    fn update(document: &mut Document, grid: NodeOf<Self>, node: NodeId, span: usize) {
        document.set_grid_span(grid, node, span);
    }
}

impl ParentKind for LayersNode {
    type Kind = LayersKind;

    fn item(node: NodeId, _: ()) -> NodeId {
        node
    }
}

impl ParentKind for CanvasNode {
    type Kind = CanvasKind;

    fn item(node: NodeId, _: ()) -> NodeId {
        node
    }
}

impl ParentKind for TextNode {
    type Kind = TextKind;

    fn item(node: NodeId, anchored: bool) -> TextPart {
        match anchored {
            true => TextPart::Anchored(node),
            false => TextPart::Inline(node),
        }
    }
}

pub struct TextKind;

impl beui_types::tree::ParentKind for TextKind {
    type Data = bool;
}

pub use beui_types::tree::{
    BareChild, CanvasKind, FromPlaced, GridKind, LayersKind, ListKind, Placed, PlacedItem,
    WithSizing, with_sizing,
};
