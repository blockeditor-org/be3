use std::ops::Range;

use super::{
    BoundaryMode, CollapsibleSection, CursorLeftRightStop, CursorStops, LRDirection, Position,
    UDDirection, VerticalMoveMode, to_boundary,
};
use crate::document::{DocumentRead, DocumentView};

/// The rows a view shows the text in, as the view decides them: the rows a
/// paragraph wraps into, or a hex view's fixed-width chunks.
/// [`super::CursorHorizontalPositionMetric::Screen`] moves carets by them.
///
/// Positions across a row (`x`) are in a unit of the view's choosing that stays
/// steady between moves; the text area uses unscaled pixels.
pub trait VisualRows {
    /// Where across its row the caret before byte `index` is drawn, or `None` when
    /// the view cannot place it now (its row is not laid out).
    fn caret_x(&self, index: usize) -> Option<f32>;

    /// The row before (`Up`) or after (`Down`) the row the caret at `index` is drawn
    /// in, skipping rows the view hides.
    fn step(&self, index: usize, direction: UDDirection, x: f32) -> RowStep;
}

pub enum RowStep {
    /// The next row in that direction.
    Row {
        /// The bytes the row shows.
        bytes: Range<usize>,
        /// The caret position in the row drawn nearest `x`.
        nearest: usize,
        /// The next row starts at `bytes.end` (a soft wrap, a fixed-width chunk), so a
        /// caret there is drawn on that row: the core settles one stop before it.
        wrapped_end: bool,
    },
    /// The caret is in the first (`Up`) or last (`Down`) row; it moves to the start
    /// or end of the text.
    Edge,
    /// The view cannot tell now, say because the row is not built; the caret moves
    /// by source line instead.
    Unknown,
}

/// Where a run of vertical moves began, so repeated moves keep aiming for the same
/// column even through shorter rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerticalMoveStart {
    pub position: Position,
    pub mode: VerticalMoveMode,
    /// The view's [`VisualRows::caret_x`] for the start, once a view placed it; held
    /// as bits so cursors stay `Eq`.
    x: Option<u32>,
}

impl VerticalMoveStart {
    pub fn new(position: Position, mode: VerticalMoveMode, x: Option<f32>) -> Self {
        Self {
            position,
            mode,
            x: x.map(f32::to_bits),
        }
    }

    pub fn x(&self) -> Option<f32> {
        self.x.map(f32::from_bits)
    }
}

/// Where the caret at `focus` lands one row up or down by the view's rows, or `None`
/// when the view cannot tell and the caret should move by source line.
#[allow(clippy::too_many_arguments)]
pub(super) fn step_row(
    document: &DocumentView<'_>,
    sections: &[CollapsibleSection],
    rows: &dyn VisualRows,
    focus: usize,
    direction: UDDirection,
    x: f32,
    stop: CursorLeftRightStop,
    stops: CursorStops,
) -> Option<usize> {
    let len = document.len();
    let (row, nearest, wrapped_end) = match rows.step(focus, direction, x) {
        RowStep::Unknown => return None,
        RowStep::Edge => {
            return Some(match direction {
                UDDirection::Up => 0,
                UDDirection::Down => len,
            });
        }
        RowStep::Row {
            bytes,
            nearest,
            wrapped_end,
        } => (bytes, nearest, wrapped_end),
    };
    let end = row.end.min(len);
    let start = row.start.min(end);
    let nearest = nearest.clamp(start, end);
    let settled = match wrapped_end && nearest == end && end > start {
        true => to_boundary(
            document.bytes(),
            end,
            LRDirection::Left,
            stop,
            BoundaryMode::Select,
            false,
            stops,
        ),
        false => to_boundary(
            document.bytes(),
            nearest,
            LRDirection::Left,
            stop,
            BoundaryMode::Select,
            true,
            stops,
        ),
    }
    .max(start);
    let hidden = sections.iter().any(|section| {
        section.collapsed && settled > section.line_end && settled <= section.content_end
    });
    (!hidden).then_some(settled)
}
