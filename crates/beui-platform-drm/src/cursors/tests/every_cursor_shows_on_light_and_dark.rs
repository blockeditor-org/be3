use beui::{Rect, pos2, vec2};

use super::*;

const CELL: f32 = 56.0;
const COLUMNS: usize = 7;

#[test]
fn every_cursor_shows_on_light_and_dark() {
    let rows = CursorIcon::ALL.len().div_ceil(COLUMNS);
    let half = rows as f32 * CELL;
    let size = vec2(COLUMNS as f32 * CELL, 2.0 * half);
    let context = beui_host::native::context();
    let output = context.run(|context| {
        let painter = context.painter();
        for (top, background) in [(0.0, Color32::WHITE), (half, Color32::from_gray(24))] {
            painter.rect_filled(
                Rect::from_min_size(pos2(0.0, top), vec2(size.x, half)),
                0.0,
                background,
            );
            for (index, icon) in CursorIcon::ALL.into_iter().enumerate() {
                let (row, column) = (index / COLUMNS, index % COLUMNS);
                let at = pos2(
                    (column as f32 + 0.5) * CELL,
                    top + (row as f32 + 0.5) * CELL,
                );
                drawn_cursor::paint(&painter, at, icon, COLOR);
            }
        }
    });
    let painting = beui_core::painting::painting(
        output.main(),
        size,
        1.0,
        Color32::TRANSPARENT,
        beui_core::painting::Punches::Shown,
    )
    .expect("the cursors paint");
    beui_paint_snapshot::assert_painting("every_cursor_shows_on_light_and_dark", painting);
}
