use crate::reactive::{
    BuildsNode, Child, ChildSegment, ChildValue, Children, IntoChild, IntoSegment, NodeSlot, Prop,
    Scope, SlotChild, create_effect, with_document,
};
use beui_core::base::grid::{GridItem, GridNode, Track};
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_macros::component;

pub struct GridCell {
    node: NodeId,
    span: Prop<usize>,
}

impl BuildsNode for GridCell {
    fn built_node(&self) -> NodeId {
        self.node
    }
}

impl ChildValue for GridCell {
    fn anchor(&self) -> Option<NodeId> {
        Some(self.node)
    }

    fn adopt_scope(&mut self, scope: Scope) {
        self.node.adopt_scope(scope);
    }
}

crate::child_type!(GridCell);

impl IntoChild<GridCell> for NodeId {
    fn into_child(self) -> GridCell {
        GridCell {
            node: self,
            span: Prop::Static(1),
        }
    }
}

impl IntoSegment<GridCell> for NodeId {
    fn into_segment(self) -> ChildSegment<GridCell> {
        ChildSegment::One(self.into_child())
    }
}

impl SlotChild for GridCell {
    type Stored = GridItem;

    fn store(self, parent: Option<NodeId>) -> GridItem {
        let GridCell { node, span } = self;
        let initial = span.peek();
        if let (Some(parent), Prop::Dynamic(read)) = (parent, span) {
            create_effect(move || {
                let span = read();
                with_document(|document| {
                    if let Some(grid) = document.arena.kind_of::<GridNode>(parent) {
                        document.set_grid_span(grid, node, span);
                    }
                });
            });
        }
        GridItem {
            child: node,
            span: initial,
        }
    }

    fn stored_node(stored: &GridItem) -> Option<NodeId> {
        Some(stored.child)
    }
}

impl NodeSlot for GridCell {
    type Host = GridNode;
}

#[component]
pub fn Grid(
    columns: Prop<Vec<Track>>,
    #[prop(default = 0.0)] column_spacing: Prop<f32>,
    #[prop(default = 0.0)] row_spacing: Prop<f32>,
    children: Children<GridCell>,
) -> NodeId {
    let grid = with_document(Document::create_grid);
    create_effect(move || {
        let columns = columns.get();
        with_document(|document| document.set_grid_columns(grid, columns));
    });
    create_effect(move || {
        let spacing = (column_spacing.get(), row_spacing.get());
        with_document(|document| document.set_grid_spacing(grid, spacing.0, spacing.1));
    });
    children.mount(grid);
    grid.id()
}

#[component]
pub fn GridCell(#[prop(default = 1)] span: Prop<usize>, children: Child) -> GridCell {
    GridCell {
        node: children,
        span,
    }
}
