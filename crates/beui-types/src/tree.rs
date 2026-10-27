use beui_tree::reactive::{
    ChildSegment, ChildValue, Children, ComponentContext, Get, IntoChild, IntoProp, IntoSegment,
    Prop, ScopeContext, SlotChild,
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
    type Stored = PlacedItem<P>;

    fn store(self) -> PlacedItem<P> {
        PlacedItem {
            node: self.node,
            data: self.data.get_untracked(),
        }
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

pub struct ListKind;

impl ParentKind for ListKind {
    type Data = Sizing;
}

impl BareChild for ListKind {
    fn bare() -> Sizing {
        Sizing::default()
    }
}

pub type ListChild = Placed<ListKind>;

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
