use crate::reactive::Child;
use crate::reactive::Children;
use crate::reactive::Get;
use crate::reactive::NodeId;
use crate::reactive::Prop;
use crate::reactive::create_binding;
use crate::types::Track;
use crate::types::tree::GridCell;
use beui_core_html::css;
use beui_core_html::document::{create, set_style};
use beui_core_html::tree::{GridKind, host};

pub(crate) fn grid(
    columns: Prop<Vec<Track>>,
    column_spacing: Prop<f32>,
    row_spacing: Prop<f32>,
    children: Children<GridCell>,
) -> NodeId {
    let grid = create("Grid", "div");
    set_style(grid, "display", Some("grid".to_owned()));
    set_style(grid, "align-content", Some("start".to_owned()));
    create_binding(move || {
        let columns = columns.get();
        set_style(grid, "grid-template-columns", Some(css::tracks(&columns)));
    });
    create_binding(move || {
        let (column, row) = (column_spacing.get(), row_spacing.get());
        set_style(grid, "column-gap", (column != 0.0).then(|| css::px(column)));
        set_style(grid, "row-gap", (row != 0.0).then(|| css::px(row)));
    });
    children.mount(host::<GridKind>(grid));
    grid
}

pub(crate) fn grid_cell(span: Prop<usize>, children: Child) -> GridCell {
    GridCell {
        node: children,
        data: span,
    }
}
