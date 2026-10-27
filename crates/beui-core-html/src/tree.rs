use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui_html::Node;
use beui_tree::reactive::{
    ComponentContext, Get, IntoProp, IntoSlotHost, Prop, ReadSignal, ScopeContext, Set, Signal,
    SlotHost, SlotId, create_effect, create_signal, current_component,
};
pub use beui_types::node::BuildsNode;
use beui_types::node::{CoreHooks, install_core_hooks};
pub use beui_types::tree::{
    BareChild, CanvasKind, FromPlaced, GridKind, LayersKind, ListKind, Placed, PlacedItem,
    WithSizing, with_sizing,
};
use beui_types::{Align, Direction, ItemSize, Sizing};

use crate::css::px;
use crate::document::{self, element, set_placed_style};
use crate::node::{NodeId, NodeRef};

#[derive(Default)]
struct NodeBindings {
    target: Cell<Option<NodeId>>,
    states: RefCell<Vec<Box<dyn Any>>>,
    accessibility: RefCell<Option<accesskit::Node>>,
    placed: RefCell<Option<Signal<bool>>>,
    node: RefCell<Option<NodeRef>>,
}

fn bindings() -> Rc<NodeBindings> {
    current_component().extension::<NodeBindings>()
}

pub fn component_node() -> NodeRef {
    let bindings = bindings();
    if let Some(node) = bindings.node.borrow().as_ref() {
        return *node;
    }
    let node = NodeRef::new();
    if let Some(target) = bindings.target.get() {
        node.fill(target);
    }
    bindings.node.replace(Some(node));
    node
}

pub fn component_placed() -> ReadSignal<bool> {
    let bindings = bindings();
    if let Some(target) = bindings.target.get() {
        return crate::frame::watch_placed(target);
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
        Some(target) => document::set_state(target, Box::new(state)),
        None => bindings.states.borrow_mut().push(Box::new(state)),
    }
}

pub fn component_accessibility(node: impl IntoProp<accesskit::Node>) {
    let bindings = bindings();
    let node = node.into_prop();
    create_effect(move || {
        let node = node.get();
        match bindings.target.get() {
            Some(target) => document::set_accessibility(target, node),
            None => {
                bindings.accessibility.replace(Some(node));
            }
        }
    });
}

fn finish_component(root: NodeId, component: &ComponentContext, scope: &ScopeContext) {
    let Some(bindings) = component.find_extension::<NodeBindings>() else {
        return;
    };
    bindings.target.set(Some(root));
    if let Some(node) = bindings.node.borrow().as_ref() {
        node.fill(root);
    }
    if let Some(placed) = bindings.placed.take() {
        let watched = crate::frame::watch_placed(root);
        scope.run(|| create_effect(move || placed.set(watched.get())));
    }
    for state in bindings.states.take() {
        document::set_state(root, state);
    }
    if let Some(node) = bindings.accessibility.take() {
        document::set_accessibility(root, node);
    }
}

pub(crate) fn install_hooks() {
    install_core_hooks(CoreHooks {
        fill_ref: document::fill_ref,
        finish_component,
    });
}

pub trait ParentKind: beui_types::tree::ParentKind {
    fn place(parent: NodeId, child: NodeId, data: Self::Data);
}

struct ElementHost<P> {
    parent: NodeId,
    inner: Rc<dyn SlotHost<Node>>,
    kind: std::marker::PhantomData<fn() -> P>,
}

fn elements(nodes: Vec<NodeId>) -> Vec<Node> {
    nodes.into_iter().filter_map(element).collect()
}

impl<P: ParentKind> SlotHost<Placed<P>> for ElementHost<P> {
    fn store(&self, child: Placed<P>) -> PlacedItem<P> {
        let Placed { node, data } = child;
        let parent = self.parent;
        let initial = data.get_untracked();
        match data {
            Prop::Static(data) => P::place(parent, node, data),
            Prop::Dynamic(read) => {
                create_effect(move || P::place(parent, node, read.get()));
            }
        }
        PlacedItem {
            node,
            data: initial,
        }
    }

    fn append(&self, stored: PlacedItem<P>) {
        if let Some(element) = element(stored.node) {
            self.inner.append(element);
        }
    }

    fn open(&self) -> SlotId {
        self.inner.open()
    }

    fn fill(&self, slot: SlotId, items: Vec<PlacedItem<P>>) {
        self.inner.fill(
            slot,
            elements(items.into_iter().map(|item| item.node).collect()),
        );
    }
}

pub fn host<P: ParentKind>(parent: NodeId) -> Rc<dyn SlotHost<Placed<P>>> {
    let element = element(parent).expect("a parent is built before its children");
    Rc::new(ElementHost::<P> {
        parent,
        inner: IntoSlotHost::<Node>::into_slot_host(&element),
        kind: std::marker::PhantomData,
    })
}

pub type ListChild = Placed<ListKind>;

fn align_css(align: Align) -> &'static str {
    match align {
        Align::Start => "start",
        Align::Center => "center",
        Align::End => "end",
        Align::Stretch => "stretch",
        Align::Baseline => "baseline",
    }
}

impl ParentKind for ListKind {
    fn place(parent: NodeId, child: NodeId, sizing: Sizing) {
        let (min, max) = match document::direction(parent) {
            Direction::Horizontal => ("min-width", "max-width"),
            Direction::Vertical => ("min-height", "max-height"),
        };
        let flex = match sizing.size {
            ItemSize::Intrinsic => format!("0 {} auto", sizing.shrink),
            ItemSize::Fixed(length) => format!("0 {} {}", sizing.shrink, px(length)),
            ItemSize::Percent(share) => format!("{share} {} 0%", sizing.shrink),
        };
        set_placed_style(child, "flex", Some(flex));
        let held_at_content = sizing.size == ItemSize::Intrinsic && sizing.shrink <= 0.0;
        set_placed_style(
            child,
            min,
            (sizing.min > 0.0 || !held_at_content).then(|| px(sizing.min)),
        );
        set_placed_style(child, max, sizing.max.is_finite().then(|| px(sizing.max)));
        set_placed_style(
            child,
            "align-self",
            sizing.align.map(|align| align_css(align).to_owned()),
        );
    }
}

impl ParentKind for GridKind {
    fn place(_parent: NodeId, child: NodeId, span: usize) {
        set_placed_style(
            child,
            "grid-column",
            (span > 1).then(|| format!("span {span}")),
        );
    }
}

impl ParentKind for LayersKind {
    fn place(_parent: NodeId, child: NodeId, _: ()) {
        set_placed_style(child, "grid-area", Some("1 / 1".to_owned()));
    }
}

impl ParentKind for CanvasKind {
    fn place(parent: NodeId, child: NodeId, _: ()) {
        crate::canvas::place(parent, child);
    }
}

pub struct FitKind;

impl beui_types::tree::ParentKind for FitKind {
    type Data = ();
}

impl ParentKind for FitKind {
    fn place(_parent: NodeId, _child: NodeId, _: ()) {}
}

impl BareChild for FitKind {
    fn bare() {}
}

pub struct TextKind;

impl beui_types::tree::ParentKind for TextKind {
    type Data = crate::text::TextSlot;
}

impl ParentKind for TextKind {
    fn place(_parent: NodeId, _child: NodeId, _slot: crate::text::TextSlot) {}
}

struct NodeHost {
    inner: Rc<dyn SlotHost<Node>>,
}

impl SlotHost<NodeId> for NodeHost {
    fn append(&self, stored: NodeId) {
        if let Some(element) = element(stored) {
            self.inner.append(element);
        }
    }

    fn open(&self) -> SlotId {
        self.inner.open()
    }

    fn fill(&self, slot: SlotId, items: Vec<NodeId>) {
        self.inner.fill(slot, elements(items));
    }
}

pub fn node_host(parent: NodeId) -> Rc<dyn SlotHost<NodeId>> {
    let element = element(parent).expect("a parent is built before its children");
    Rc::new(NodeHost {
        inner: IntoSlotHost::<Node>::into_slot_host(&element),
    })
}
