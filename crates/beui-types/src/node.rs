use std::cell::Cell;
use std::rc::Weak;
use std::sync::OnceLock;

use beui_tree::reactive::{
    AtMostOneChild, ChildSegment, ChildValue, Children, ComponentContext, ComponentOutput,
    CountOne, Counted, HostedNode, IntoChild, IntoSegment, LazyChild, OneOf, ScopeContext,
    SingleChild, SlotChild, Stored, ZeroOrOne, store,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId {
    index: u32,
    generation: u32,
}

impl NodeId {
    pub const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    pub fn index(self) -> u32 {
        self.index
    }

    pub fn generation(self) -> u32 {
        self.generation
    }

    pub fn key(self) -> u64 {
        (u64::from(self.generation) << 32) | u64::from(self.index)
    }

    pub fn from_key(key: u64) -> Self {
        Self {
            index: key as u32,
            generation: (key >> 32) as u32,
        }
    }
}

pub type NodeCell = Cell<Option<NodeId>>;

pub struct CoreHooks {
    pub fill_ref: fn(NodeId, Weak<NodeCell>),
    pub finish_component: fn(NodeId, &ComponentContext, &ScopeContext),
    /// Builds a host node holding `child`, which control flow swaps (`create_host`).
    pub create_host: fn(LazyChild<NodeId>) -> NodeId,
}

static HOOKS: OnceLock<CoreHooks> = OnceLock::new();

pub fn install_core_hooks(hooks: CoreHooks) {
    let _ = HOOKS.set(hooks);
}

/// A node with no look of its own that sizes to `child` and swaps it in place: what control
/// flow becomes in a slot that takes one node, or when it is named with `@test_id` or
/// `@node_ref`. The core builds it (`CoreHooks::create_host`).
pub fn create_host(child: LazyChild<NodeId>) -> NodeId {
    let hooks = HOOKS
        .get()
        .expect("a core is installed before control flow is placed in a node's slot");
    (hooks.create_host)(child)
}

impl IntoChild<NodeId> for OneOf<NodeId> {
    fn into_child(self) -> NodeId {
        create_host(self.into_lazy())
    }
}

impl HostedNode for NodeId {}

impl AtMostOneChild<NodeId> for SingleChild<ZeroOrOne<NodeId>> {
    fn at_most_one_child(self) -> Option<NodeId> {
        Some(create_host(self.0.into_lazy()))
    }
}

impl ComponentOutput<NodeId> for OneOf<NodeId> {
    fn into_output(self) -> NodeId {
        create_host(self.into_lazy())
    }
}

impl ComponentOutput<NodeId> for ZeroOrOne<NodeId> {
    fn into_output(self) -> NodeId {
        create_host(self.into_lazy())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NodeRef(Stored<NodeCell>);

impl Default for NodeRef {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeRef {
    pub fn new() -> Self {
        Self(store(Cell::new(None)))
    }

    pub fn fill(&self, node: NodeId) {
        let Some(cell) = self.0.try_value() else {
            return;
        };
        cell.set(Some(node));
        if let Some(hooks) = HOOKS.get() {
            (hooks.fill_ref)(node, std::rc::Rc::downgrade(&cell));
        }
    }

    pub fn get(&self) -> NodeId {
        self.try_get()
            .expect("node_ref read while the node it points at is not built")
    }

    pub fn try_get(&self) -> Option<NodeId> {
        self.0.try_with(Cell::get).flatten()
    }
}

impl ChildValue for NodeId {
    fn finish_component(&mut self, component: &ComponentContext, scope: &ScopeContext) {
        if let Some(hooks) = HOOKS.get() {
            (hooks.finish_component)(*self, component, scope);
        }
    }
}

impl SlotChild for NodeId {
    type Stored = NodeId;
    type Node = NodeId;

    fn node(stored: &NodeId) -> NodeId {
        *stored
    }

    fn store(self) -> NodeId {
        self
    }
}

impl IntoChild<NodeId> for NodeId {
    fn into_child(self) -> NodeId {
        self
    }
}

impl Counted for NodeId {
    type Count = CountOne;
}

impl IntoSegment<NodeId> for NodeId {
    fn into_segment(self) -> ChildSegment<NodeId> {
        ChildSegment::One(self)
    }
}

impl IntoSegment<NodeId> for Option<NodeId> {
    fn into_segment(self) -> ChildSegment<NodeId> {
        ChildSegment::Many(self.into_iter().collect())
    }
}

impl From<NodeId> for Children<NodeId> {
    fn from(node: NodeId) -> Self {
        Self::from(ChildSegment::One(node))
    }
}

#[diagnostic::on_unimplemented(
    message = "a `{Self}` is no node for `@node_ref` or `@test_id` to name",
    label = "put the modifier on a tag that builds a node, inside this one or around it; `@test_id` also names a value child its parent builds a row for"
)]
pub trait BuildsNode {
    fn built_node(&self) -> NodeId;
}

/// What `@node_ref` names (and `@test_id`, through beui-view's `TakesTestId`): a node, or
/// control flow that builds at most one, which then becomes a host node (`create_host`)
/// wherever it goes, even among a run.
#[diagnostic::on_unimplemented(
    message = "a `{Self}` is no node for `@node_ref` or `@test_id` to name",
    label = "a run of children has no node of its own: put the modifier on a tag inside each row, or on a tag around the run; `@test_id` also names a value child its parent builds a row for"
)]
pub trait NamesNode {
    type Named;

    fn name_node(self) -> (Self::Named, NodeId);
}

impl<T: BuildsNode> NamesNode for T {
    type Named = T;

    fn name_node(self) -> (T, NodeId) {
        let node = self.built_node();
        (self, node)
    }
}

impl NamesNode for OneOf<NodeId> {
    type Named = NodeId;

    fn name_node(self) -> (NodeId, NodeId) {
        let host = create_host(self.into_lazy());
        (host, host)
    }
}

impl NamesNode for ZeroOrOne<NodeId> {
    type Named = NodeId;

    fn name_node(self) -> (NodeId, NodeId) {
        let host = create_host(self.into_lazy());
        (host, host)
    }
}

impl BuildsNode for NodeId {
    fn built_node(&self) -> NodeId {
        *self
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ImeCursor {
    pub node: NodeId,
    pub rect: Option<be_lib_geometry::Rect>,
}
