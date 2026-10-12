use crate::reactive::{Child, Children, Placed, Prop, create_effect, with_document};
use beui_core::base::grid::{GridNode, Track};
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_macros::component;

pub type GridCell = Placed<GridNode>;

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
        data: span,
    }
}
