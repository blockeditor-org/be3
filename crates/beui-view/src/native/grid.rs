use crate::reactive::Child;
use crate::reactive::Children;
use crate::reactive::Get;
use crate::reactive::Placed;
use crate::reactive::Prop;
use crate::reactive::create_effect;
use crate::reactive::with_document;
use beui_core::base::grid::GridNode;
use beui_core::base::grid::Track;
use beui_core::document::Document;
use beui_core::node::NodeId;

pub type GridCell = Placed<GridNode>;

pub fn grid(
    columns: Prop<Vec<Track>>,
    column_spacing: Prop<f32>,
    row_spacing: Prop<f32>,
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

pub fn grid_cell(span: Prop<usize>, children: Child) -> GridCell {
    GridCell {
        node: children,
        data: span,
    }
}
