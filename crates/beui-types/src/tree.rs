use beui_tree::reactive::{
    ChildSegment, ChildValue, Children, ComponentContext, CountOne, Counted, Get, IntoChild,
    IntoPropValue, IntoSegment, OneOf, Prop, ScopeContext, SlotChild, ZeroOrOne,
};

use crate::layout::Sizing;
use crate::node::{BuildsNode, NodeId};

pub trait ParentKind: 'static {
    type Data: Clone + 'static;
}

pub trait BareChild: ParentKind {
    fn bare() -> Self::Data;
}

pub struct Placed<P: ParentKind> {
    pub node: NodeId,
    pub data: Prop<P::Data>,
}

pub struct PlacedItem<P: ParentKind> {
    pub node: NodeId,
    pub data: P::Data,
}

impl<P: ParentKind> Clone for PlacedItem<P> {
    fn clone(&self) -> Self {
        Self {
            node: self.node,
            data: self.data.clone(),
        }
    }
}

impl<P: ParentKind> Placed<P> {
    pub fn new<M>(node: NodeId, data: impl IntoPropValue<Prop<P::Data>, M>) -> Self {
        Self {
            node,
            data: data.into_prop_value(),
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

impl<P: ParentKind> Counted for Placed<P> {
    type Count = CountOne;
}

beui_tree::one_child_slot!(<P: ParentKind> Placed<P>);

impl<P: ParentKind> SlotChild for Placed<P> {
    type Stored = PlacedItem<P>;
    type Node = NodeId;

    fn node(stored: &PlacedItem<P>) -> NodeId {
        stored.node
    }

    fn store(self) -> PlacedItem<P> {
        PlacedItem {
            node: self.node,
            data: self.data.get_untracked(),
        }
    }
}

#[diagnostic::on_unimplemented(
    message = "a child made for a `{P}` cannot be a child of this component",
    label = "a `<ListItem>` is a list child: remove it, or put this child in a `List` or `Stack`"
)]
pub trait FromPlaced<P: ParentKind> {
    fn from_placed(child: Placed<P>) -> Self;
}

impl<P: ParentKind> FromPlaced<P> for Placed<P> {
    fn from_placed(child: Placed<P>) -> Self {
        child
    }
}

/// A placed child where a bare node goes: only a parent whose children are bare nodes takes
/// one (`FromPlaced`'s message says which parent it was made for).
impl<P: ParentKind> IntoChild<NodeId> for Placed<P>
where
    NodeId: FromPlaced<P>,
{
    fn into_child(self) -> NodeId {
        NodeId::from_placed(self)
    }
}

impl<P: ParentKind, Q: ParentKind> IntoChild<Placed<Q>> for Placed<P>
where
    Placed<Q>: FromPlaced<P>,
{
    fn into_child(self) -> Placed<Q> {
        Placed::<Q>::from_placed(self)
    }
}

impl<P: ParentKind, T: FromPlaced<P> + SlotChild> IntoSegment<T> for Placed<P> {
    fn into_segment(self) -> ChildSegment<T> {
        ChildSegment::One(T::from_placed(self))
    }
}

impl<P: ParentKind, T: FromPlaced<P> + SlotChild> From<Placed<P>> for Children<T> {
    fn from(child: Placed<P>) -> Self {
        Self::from(ChildSegment::One(T::from_placed(child)))
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

impl<P: BareChild> beui_tree::reactive::IntoPropValue<Children<Placed<P>>, beui_tree::reactive::Val<beui_tree::reactive::ByInto>> for NodeId {
    fn into_prop_value(self) -> Children<Placed<P>> {
        self.into()
    }
}

impl<P: ParentKind, T: FromPlaced<P> + SlotChild>
    beui_tree::reactive::IntoPropValue<Children<T>, beui_tree::reactive::Val<beui_tree::reactive::ByInto>> for Placed<P>
{
    fn into_prop_value(self) -> Children<T> {
        self.into()
    }
}

impl<P: BareChild> From<NodeId> for Children<Placed<P>> {
    fn from(node: NodeId) -> Self {
        Self::from(IntoSegment::<Placed<P>>::into_segment(node))
    }
}

pub struct ListKind;

impl ParentKind for ListKind {
    type Data = Sizing;
}

impl BareChild for ListKind {
    fn bare() -> Sizing {
        Sizing::default()
    }
}

pub type ListItem = Placed<ListKind>;

pub struct GridKind;

impl ParentKind for GridKind {
    type Data = usize;
}

impl BareChild for GridKind {
    fn bare() -> usize {
        1
    }
}

pub type GridCell = Placed<GridKind>;

pub struct LayersKind;

impl ParentKind for LayersKind {
    type Data = ();
}

impl BareChild for LayersKind {
    fn bare() {}
}

pub type Layer = Placed<LayersKind>;

pub struct CanvasKind;

impl ParentKind for CanvasKind {
    type Data = ();
}

pub type CanvasItem = Placed<CanvasKind>;

/// A row of a `ForEach` (or any slot of placed children taking one item) that is control
/// flow building at most one node: a host node holds it, one per row.
impl<P: BareChild> IntoChild<Placed<P>> for OneOf<NodeId> {
    fn into_child(self) -> Placed<P> {
        crate::node::create_host(self.into_lazy()).into_child()
    }
}

impl<P: BareChild> IntoChild<Placed<P>> for ZeroOrOne<NodeId> {
    fn into_child(self) -> Placed<P> {
        crate::node::create_host(self.into_lazy()).into_child()
    }
}
