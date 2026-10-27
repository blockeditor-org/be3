use crate::reactive::Child;
use crate::reactive::Children;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::types::Track;
pub use crate::types::tree::GridCell;
use beui_macros::component;

#[component]
pub fn Grid(
    columns: Prop<Vec<Track>>,
    #[prop(default = 0.0)] column_spacing: Prop<f32>,
    #[prop(default = 0.0)] row_spacing: Prop<f32>,
    children: Children<GridCell>,
) -> NodeId {
    crate::core::grid(columns, column_spacing, row_spacing, children)
}

#[component]
pub fn GridCell(#[prop(default = 1)] span: Prop<usize>, children: Child) -> GridCell {
    crate::core::grid_cell(span, children)
}
