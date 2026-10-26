use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui_html::Node;
use beui_tree::reactive::{
    ChildSegment, ChildValue, Children, ComponentContext, Get, IntoChild, IntoProp, IntoSegment,
    IntoSlotHost, Prop, ReadSignal, ScopeContext, Set, Signal, SlotChild, SlotHost, SlotId,
    create_effect, create_signal, current_component,
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

impl ChildValue for NodeId {
    fn finish_component(&mut self, component: &ComponentContext, scope: &ScopeContext) {
        let root = *self;
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
pub trait ParentKind: Sized + 'static {
    type Data: Clone + 'static;

    fn place(parent: NodeId, child: NodeId, data: Self::Data);
}

pub trait BareChild: ParentKind {
    fn bare() -> Self::Data;
}

pub struct Placed<P: ParentKind> {
    pub node: NodeId,
    pub data: Prop<P::Data>,
}

impl<P: ParentKind> Placed<P> {
    pub fn new(node: NodeId, data: impl IntoProp<P::Data>) -> Self {
        Self {
            node,
            data: data.into_prop(),
        }
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
    type Stored = NodeId;

    fn store(self) -> NodeId {
        self.node
    }
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
    fn store(&self, child: Placed<P>) -> NodeId {
        let Placed { node, data } = child;
        let parent = self.parent;
        match data {
            Prop::Static(data) => P::place(parent, node, data),
            Prop::Dynamic(read) => {
                create_effect(move || P::place(parent, node, read.get()));
            }
        }
        node
    }

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

pub fn host<P: ParentKind>(parent: NodeId) -> Rc<dyn SlotHost<Placed<P>>> {
    let element = element(parent).expect("a parent is built before its children");
    Rc::new(ElementHost::<P> {
        parent,
        inner: IntoSlotHost::<Node>::into_slot_host(&element),
        kind: std::marker::PhantomData,
    })
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

pub struct ListKind;

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
    type Data = Sizing;

    fn place(parent: NodeId, child: NodeId, sizing: Sizing) {
        let (min, max) = match document::direction(parent) {
            Direction::Horizontal => ("min-width", "max-width"),
            Direction::Vertical => ("min-height", "max-height"),
        };
        let flex = match sizing.size {
            ItemSize::Intrinsic => format!("0 {} auto", sizing.shrink),
            ItemSize::Fixed(length) => format!("0 {} {}", sizing.shrink, px(length)),
            ItemSize::Percent(share) => format!("{share} {} 0px", sizing.shrink),
        };
        set_placed_style(child, "flex", Some(flex));
        set_placed_style(child, min, (sizing.min > 0.0).then(|| px(sizing.min)));
        set_placed_style(child, max, sizing.max.is_finite().then(|| px(sizing.max)));
        set_placed_style(
            child,
            "align-self",
            sizing.align.map(|align| align_css(align).to_owned()),
        );
    }
}

impl BareChild for ListKind {
    fn bare() -> Sizing {
        Sizing::default()
    }
}

pub struct GridKind;

impl ParentKind for GridKind {
    type Data = usize;

    fn place(_parent: NodeId, child: NodeId, span: usize) {
        set_placed_style(
            child,
            "grid-column",
            (span > 1).then(|| format!("span {span}")),
        );
    }
}

impl BareChild for GridKind {
    fn bare() -> usize {
        1
    }
}

pub struct LayersKind;

impl ParentKind for LayersKind {
    type Data = ();

    fn place(_parent: NodeId, child: NodeId, _: ()) {
        set_placed_style(child, "grid-area", Some("1 / 1".to_owned()));
    }
}

impl BareChild for LayersKind {
    fn bare() {}
}

pub struct CanvasKind;

impl ParentKind for CanvasKind {
    type Data = ();

    fn place(_parent: NodeId, _child: NodeId, _: ()) {}
}

pub struct FitKind;

impl ParentKind for FitKind {
    type Data = ();

    fn place(_parent: NodeId, _child: NodeId, _: ()) {}
}

impl BareChild for FitKind {
    fn bare() {}
}

pub struct TextKind;

impl ParentKind for TextKind {
    type Data = bool;

    fn place(_parent: NodeId, _child: NodeId, _anchored: bool) {}
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
