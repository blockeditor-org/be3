use std::any::Any;

use crate::base::child_list::{ChildHost, ChildItem, ChildList, NodeChildren};
use crate::base::share::{Part, share, snapped_run};
use crate::document::Document;
use crate::geometry::{Rect, Vec2, pos2, vec2};
use crate::node::{Element, InteractInput, NodeId, NodeOf, Rects};
use crate::painter::Painter;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Track {
    Fixed(f32),
    Intrinsic,
    Fraction(f32),
}

#[derive(Clone, Copy)]
pub struct GridItem {
    pub child: NodeId,
    pub span: usize,
}

impl ChildItem for GridItem {
    fn node(&self) -> NodeId {
        self.child
    }
}

pub struct GridNode {
    pub columns: Vec<Track>,
    pub column_spacing: f32,
    pub row_spacing: f32,
    items: ChildList<GridItem>,
}

impl ChildHost for GridNode {
    type Stored = GridItem;

    fn children(&mut self) -> &mut ChildList<GridItem> {
        &mut self.items
    }
}

struct Cell {
    child: NodeId,
    row: usize,
    column: usize,
    span: usize,
}

struct Plan {
    cells: Vec<Cell>,
    columns: Vec<f32>,
    rows: Vec<f32>,
    column_spacing: f32,
    row_spacing: f32,
}

impl Plan {
    fn column_start(&self, column: usize) -> f32 {
        self.columns[..column].iter().sum::<f32>() + self.column_spacing * column as f32
    }

    fn span_width(&self, column: usize, span: usize) -> f32 {
        self.columns[column..column + span].iter().sum::<f32>()
            + self.column_spacing * (span - 1) as f32
    }

    fn size(&self) -> Vec2 {
        let gaps = |count: usize, spacing: f32| spacing * count.saturating_sub(1) as f32;
        vec2(
            self.columns.iter().sum::<f32>() + gaps(self.columns.len(), self.column_spacing),
            self.rows.iter().sum::<f32>() + gaps(self.rows.len(), self.row_spacing),
        )
    }
}

impl GridNode {
    fn tracks(&self) -> Vec<Track> {
        match self.columns.is_empty() {
            true => vec![Track::Fraction(1.0)],
            false => self.columns.clone(),
        }
    }

    fn cells(&self, count: usize) -> Vec<Cell> {
        let mut cells = Vec::with_capacity(self.items.iter().count());
        let mut row = 0;
        let mut column = 0;
        for item in self.items.iter() {
            let span = item.span.clamp(1, count);
            if column > 0 && column + span > count {
                row += 1;
                column = 0;
            }
            cells.push(Cell {
                child: item.child,
                row,
                column,
                span,
            });
            column += span;
            if column >= count {
                row += 1;
                column = 0;
            }
        }
        cells
    }

    fn plan(&self, doc: &mut Document, painter: &Painter, available_width: f32) -> Plan {
        let grid = doc.pixel_grid();
        let tracks = self.tracks();
        let column_spacing = grid.snap(self.column_spacing);
        let row_spacing = grid.snap(self.row_spacing);
        let cells = self.cells(tracks.len());
        let bounded = available_width.is_finite();
        let sized_by_content = |track: Track| match track {
            Track::Intrinsic => true,
            Track::Fraction(_) => !bounded,
            Track::Fixed(_) => false,
        };
        let mut columns: Vec<f32> = tracks
            .iter()
            .map(|track| match track {
                Track::Fixed(width) => grid.snap(width.max(0.0)),
                _ => 0.0,
            })
            .collect();
        let unbounded = Vec2::splat(f32::INFINITY);
        for cell in cells.iter().filter(|cell| cell.span == 1) {
            if sized_by_content(tracks[cell.column]) {
                let width = crate::layout::measure(doc, painter, cell.child, unbounded).x;
                columns[cell.column] = columns[cell.column].max(width);
            }
        }
        for cell in cells.iter().filter(|cell| cell.span > 1) {
            let spanned = cell.column..cell.column + cell.span;
            let growing: Vec<usize> = spanned
                .clone()
                .filter(|column| sized_by_content(tracks[*column]))
                .collect();
            if growing.is_empty() {
                continue;
            }
            let width = crate::layout::measure(doc, painter, cell.child, unbounded).x;
            let held =
                columns[spanned].iter().sum::<f32>() + column_spacing * (cell.span - 1) as f32;
            let extra = grid.snap(((width - held) / growing.len() as f32).max(0.0));
            for column in growing {
                columns[column] += extra;
            }
        }
        if bounded {
            let taken = columns.iter().sum::<f32>()
                + column_spacing * tracks.len().saturating_sub(1) as f32;
            let parts: Vec<Part> = tracks
                .iter()
                .map(|track| match track {
                    Track::Fraction(weight) => Part::new(*weight, 0.0, f32::INFINITY),
                    _ => Part::new(0.0, 0.0, 0.0),
                })
                .collect();
            let shares = share((available_width - taken).max(0.0), &parts);
            for (column, width) in columns.iter_mut().zip(snapped_run(grid, &shares)) {
                *column += width;
            }
        }
        let rows_count = cells.last().map_or(0, |cell| cell.row + 1);
        let mut plan = Plan {
            cells,
            columns,
            rows: vec![0.0; rows_count],
            column_spacing,
            row_spacing,
        };
        for index in 0..plan.cells.len() {
            let cell = &plan.cells[index];
            let (child, row) = (cell.child, cell.row);
            let width = plan.span_width(cell.column, cell.span);
            let height = crate::layout::measure(doc, painter, child, vec2(width, f32::INFINITY)).y;
            plan.rows[row] = plan.rows[row].max(height);
        }
        plan
    }
}

impl Element for GridNode {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        self.plan(doc, painter, available.x).size()
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        let plan = self.plan(doc, painter, available.x);
        let first = plan.cells.first()?;
        let width = plan.span_width(first.column, first.span);
        crate::layout::baseline(doc, painter, first.child, vec2(width, f32::INFINITY))
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        let plan = self.plan(doc, painter, rect.width());
        let mut row_starts = Vec::with_capacity(plan.rows.len());
        let mut top = 0.0f32;
        for height in &plan.rows {
            row_starts.push(top);
            top += height + plan.row_spacing;
        }
        for cell in &plan.cells {
            let left = plan.column_start(cell.column);
            let width = plan.span_width(cell.column, cell.span);
            let cell_rect = Rect::from_min_size(
                pos2(rect.left() + left, rect.top() + row_starts[cell.row]),
                vec2(width, plan.rows[cell.row]),
            );
            crate::layout::layout(doc, painter, cell.child, cell_rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, _rect: Rect) {
        for item in self.items.iter() {
            crate::paint::paint(doc, painter, rects, item.child);
        }
    }

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _input: &InteractInput,
        _id: NodeId,
        _rect: Rect,
        _focus_target: &mut Option<NodeId>,
        children: &mut Vec<NodeId>,
    ) {
        children.extend(self.items.iter().map(ChildItem::node));
    }

    fn children(&self) -> Vec<NodeId> {
        self.items.nodes()
    }

    fn kind(&self) -> &'static str {
        "grid"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_grid(&mut self) -> NodeOf<GridNode> {
        self.arena.insert(GridNode {
            columns: Vec::new(),
            column_spacing: 0.0,
            row_spacing: 0.0,
            items: ChildList::default(),
        })
    }

    pub fn set_grid_columns(&mut self, grid: NodeOf<GridNode>, columns: Vec<Track>) {
        if self.arena.get_as::<GridNode>(grid).columns != columns {
            self.arena.get_mut_as::<GridNode>(grid).columns = columns;
        }
    }

    pub fn set_grid_spacing(
        &mut self,
        grid: NodeOf<GridNode>,
        column_spacing: f32,
        row_spacing: f32,
    ) {
        let node = self.arena.get_as::<GridNode>(grid);
        if node.column_spacing == column_spacing && node.row_spacing == row_spacing {
            return;
        }
        let node = self.arena.get_mut_as::<GridNode>(grid);
        node.column_spacing = column_spacing;
        node.row_spacing = row_spacing;
    }

    pub fn set_grid_span(&mut self, grid: NodeOf<GridNode>, child: NodeId, span: usize) {
        let current = self
            .arena
            .get_as::<GridNode>(grid)
            .items
            .find(child)
            .map(|item| item.span);
        if current.is_none_or(|current| current == span) {
            return;
        }
        if let Some(item) = self
            .arena
            .get_mut_as::<GridNode>(grid)
            .items
            .find_mut(child)
        {
            item.span = span;
        }
    }
}
