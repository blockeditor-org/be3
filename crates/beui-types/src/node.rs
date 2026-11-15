use std::cell::Cell;
use std::rc::Weak;
use std::sync::OnceLock;

use beui_tree::reactive::{
    ChildSegment, ChildValue, Children, ComponentContext, Counted, IntoChild, IntoSegment, One,
    ScopeContext, Single, SlotChild, Stored, store, AtMostOne, DynamicSegment,
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
    /// A node with no look of its own that sizes to `child` and swaps it in place.
    pub host: fn(Single<NodeId>) -> NodeId,
}

static HOOKS: OnceLock<CoreHooks> = OnceLock::new();

pub fn install_core_hooks(hooks: CoreHooks) {
    let _ = HOOKS.set(hooks);
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

    fn store(self) -> NodeId {
        self
    }
}

impl IntoChild<NodeId> for NodeId {
    fn into_child(self) -> NodeId {
        self
    }
}

impl IntoChild<Single<NodeId>> for NodeId {
    fn into_child(self) -> Single<NodeId> {
        Single::built(self)
    }
}

impl Counted for NodeId {
    type Count = One;
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

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ImeCursor {
    pub node: NodeId,
    pub rect: Option<be_lib_geometry::Rect>,
}

/// Control flow that builds at most one node, in a slot that needs a node built once
/// (`ListItem`, `Root`, `Offset`): the core builds a bare frame that sizes to its child and
/// swaps that child in place.
impl<N: AtMostOne> IntoChild<NodeId> for DynamicSegment<NodeId, N> {
    fn into_child(self) -> NodeId {
        host(Single::from(self))
    }
}

pub fn host(child: Single<NodeId>) -> NodeId {
    let hooks = HOOKS
        .get()
        .expect("a core is installed before control flow is placed in a node's slot");
    (hooks.host)(child)
}
