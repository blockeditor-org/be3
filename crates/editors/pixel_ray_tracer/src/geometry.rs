use block_client::blocks::pixel_ray_tracer::{PIXEL_RAY_TRACER_SIZE, Point};
use block_editor_plugin::beui::{Pos2, Rect, Vec2};

pub(crate) fn artwork_rect(view: Rect) -> Rect {
    let side = view.width().min(view.height());
    let centre = view.center();
    Rect::from_min_size(
        Pos2::new(centre.x - side / 2.0, centre.y - side / 2.0),
        Vec2::splat(side),
    )
}

pub(crate) fn screen_to_world(point: Pos2, canvas: Rect) -> Point {
    let scale = f32::from(PIXEL_RAY_TRACER_SIZE) / canvas.width().max(f32::EPSILON);
    let value = (point - canvas.min) * scale;
    Point::new(value.x, value.y)
}

pub(crate) fn pixel_at(point: Point, clamp: bool) -> Option<(u16, u16)> {
    if !clamp && !inside(point) {
        return None;
    }
    Some((
        point
            .x
            .floor()
            .clamp(0.0, f32::from(PIXEL_RAY_TRACER_SIZE - 1)) as u16,
        point
            .y
            .floor()
            .clamp(0.0, f32::from(PIXEL_RAY_TRACER_SIZE - 1)) as u16,
    ))
}

pub(crate) fn snap(point: Point) -> Point {
    Point::new(
        ((point.x * 2.0).round() / 2.0).clamp(0.0, f32::from(PIXEL_RAY_TRACER_SIZE)),
        ((point.y * 2.0).round() / 2.0).clamp(0.0, f32::from(PIXEL_RAY_TRACER_SIZE)),
    )
}

pub(crate) fn inside(point: Point) -> bool {
    point.x >= 0.0
        && point.y >= 0.0
        && point.x <= f32::from(PIXEL_RAY_TRACER_SIZE)
        && point.y <= f32::from(PIXEL_RAY_TRACER_SIZE)
}

pub(crate) fn distance(first: Point, second: Point) -> f32 {
    (first.x - second.x).hypot(first.y - second.y)
}

pub(crate) fn distance_to_segment(point: Point, start: Point, end: Point) -> f32 {
    let length = (end.x - start.x).powi(2) + (end.y - start.y).powi(2);
    if length == 0.0 {
        return distance(point, start);
    }
    let t = (((point.x - start.x) * (end.x - start.x) + (point.y - start.y) * (end.y - start.y))
        / length)
        .clamp(0.0, 1.0);
    distance(
        point,
        Point::new(
            start.x + t * (end.x - start.x),
            start.y + t * (end.y - start.y),
        ),
    )
}

pub(crate) fn raster_line(start: (u16, u16), end: (u16, u16)) -> Vec<(u16, u16)> {
    let (mut x, mut y) = (i32::from(start.0), i32::from(start.1));
    let (end_x, end_y) = (i32::from(end.0), i32::from(end.1));
    let dx = (end_x - x).abs();
    let sx = if x < end_x { 1 } else { -1 };
    let dy = -(end_y - y).abs();
    let sy = if y < end_y { 1 } else { -1 };
    let mut error = dx + dy;
    let mut points = Vec::new();
    loop {
        points.push((x as u16, y as u16));
        if x == end_x && y == end_y {
            break;
        }
        let twice = 2 * error;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
    }
    points
}
