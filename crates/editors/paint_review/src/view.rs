use block_editor_plugin::beui::{Rect, Vec2};

use crate::render::Rendered;

const GAP: f32 = 12.0;

pub struct Panel {
    pub rect: Rect,
    pub index: usize,
}

pub struct Layout {
    pub panels: Vec<Panel>,
    pub scale: f32,
}

pub fn laid_out(panels: &[Rendered], view: Rect) -> Layout {
    let content = content_size(panels);
    let scale = (view.width() / content.x)
        .min(view.height() / content.y)
        .max(f32::EPSILON);
    let origin = view.center() - content * (scale / 2.0);
    let mut placed = Vec::with_capacity(panels.len());
    let mut x = 0.0;
    for (index, panel) in panels.iter().enumerate() {
        let size = panel.size * scale;
        let top = origin.y + (content.y * scale - size.y) / 2.0;
        placed.push(Panel {
            rect: Rect::from_min_size(block_editor_plugin::beui::pos2(origin.x + x, top), size),
            index,
        });
        x += size.x + GAP * scale;
    }
    Layout {
        panels: placed,
        scale,
    }
}

fn content_size(panels: &[Rendered]) -> Vec2 {
    let width: f32 = panels.iter().map(|panel| panel.size.x).sum::<f32>()
        + GAP * panels.len().saturating_sub(1) as f32;
    let height = panels.iter().map(|panel| panel.size.y).fold(1.0, f32::max);
    Vec2::new(width.max(1.0), height)
}

#[cfg(test)]
mod tests;
