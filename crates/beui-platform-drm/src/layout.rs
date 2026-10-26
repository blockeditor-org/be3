use beui::{Pos2, Rect, Vec2, pos2, vec2};

use crate::rotation::{Rotation, Turn};

pub fn arrange(sizes: &[((u32, u32), f32)]) -> Vec<Rect> {
    let points: Vec<Vec2> = sizes
        .iter()
        .map(|&((width, height), scale)| vec2(width as f32 / scale, height as f32 / scale))
        .collect();
    let tallest = points.iter().map(|size| size.y).fold(0.0, f32::max);
    let mut left = 0.0;
    points
        .into_iter()
        .map(|size| {
            let rect = Rect::from_min_size(pos2(left, tallest - size.y), size);
            left += size.x;
            rect
        })
        .collect()
}

pub fn absolute(normalized: Pos2, rect: Rect, rotation: Rotation) -> Pos2 {
    let size = rect.size();
    let panel = rotation.panel_size(size);
    let on_panel = pos2(normalized.x * panel.x, normalized.y * panel.y);
    rect.min + rotation.to_picture(size, on_panel).to_vec2()
}

pub fn bounds(outputs: &[Rect]) -> Rect {
    outputs
        .iter()
        .copied()
        .reduce(|a, b| a.union(b))
        .unwrap_or(Rect::from_min_size(Pos2::ZERO, vec2(1.0, 1.0)))
}

pub fn clamp(position: Pos2, outputs: &[Rect]) -> Pos2 {
    if outputs.iter().any(|rect| inside(*rect, position)) {
        return position;
    }
    outputs
        .iter()
        .map(|rect| nearest(*rect, position))
        .min_by(|a, b| {
            let da = (*a - position).length();
            let db = (*b - position).length();
            da.total_cmp(&db)
        })
        .unwrap_or(position)
}

fn inside(rect: Rect, position: Pos2) -> bool {
    position.x >= rect.left()
        && position.y >= rect.top()
        && position.x < rect.right()
        && position.y < rect.bottom()
}

fn nearest(rect: Rect, position: Pos2) -> Pos2 {
    let last = |low: f32, high: f32| (high - 1.0).max(low);
    pos2(
        position
            .x
            .clamp(rect.left(), last(rect.left(), rect.right())),
        position
            .y
            .clamp(rect.top(), last(rect.top(), rect.bottom())),
    )
}

pub fn moved(position: Pos2, delta: Vec2, outputs: &[Rect]) -> Pos2 {
    let target = position + delta;
    if outputs.iter().any(|rect| inside(*rect, target)) {
        return target;
    }
    match outputs.iter().find(|rect| inside(**rect, position)) {
        Some(rect) => nearest(*rect, target),
        None => clamp(target, outputs),
    }
}

#[cfg(test)]
mod tests;
