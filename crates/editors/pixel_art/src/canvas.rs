use block_editor_plugin::beui::{Pos2, Rect, Vec2};

pub const ZOOM_STEP: f32 = 1.25;

pub fn canvas_rect(view: Rect, width: u16, height: u16) -> Rect {
    let scale = (view.width() / f32::from(width))
        .min(view.height() / f32::from(height))
        .max(f32::EPSILON);
    let size = Vec2::new(f32::from(width) * scale, f32::from(height) * scale);
    Rect::from_min_size(
        Pos2::new(
            view.center().x - size.x / 2.0,
            view.center().y - size.y / 2.0,
        ),
        size,
    )
}

pub fn pixel_at(position: Pos2, rect: Rect, width: u16, height: u16) -> Option<(u16, u16)> {
    if !rect.contains(position) {
        return None;
    }
    let x = (((position.x - rect.left()) / rect.width()) * f32::from(width)).floor() as u16;
    let y = (((position.y - rect.top()) / rect.height()) * f32::from(height)).floor() as u16;
    Some((x.min(width - 1), y.min(height - 1)))
}

#[cfg(test)]
mod tests;
