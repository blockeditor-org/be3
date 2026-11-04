use crate::{Pos2, Rect, Vec2};

const SHARED: f32 = 64.0;
const NEAR: f32 = 0.01;

pub fn arrange(rects: &[(Vec2, Option<Pos2>)]) -> Vec<Rect> {
    let mut placed: Vec<Rect> = Vec::with_capacity(rects.len());
    for &(size, at) in rects {
        let rect = match bounds(&placed) {
            None => Rect::from_min_size(at.unwrap_or(Pos2::ZERO), size),
            Some(bounds) => {
                let beside = Pos2::new(bounds.right(), bounds.bottom() - size.y);
                settle(Rect::from_min_size(at.unwrap_or(beside), size), &placed)
            }
        };
        placed.push(rect);
    }
    let corner = bounds(&placed).map_or(Pos2::ZERO, |bounds| bounds.min);
    placed
        .into_iter()
        .map(|rect| rect.translate(-corner.to_vec2()))
        .collect()
}

pub fn settle(rect: Rect, placed: &[Rect]) -> Rect {
    if placed.is_empty()
        || (!overlaps_any(rect, placed) && placed.iter().any(|other| touch(rect, *other)))
    {
        return rect;
    }
    let size = rect.size();
    placed
        .iter()
        .flat_map(|other| beside(rect, *other))
        .map(|min| Rect::from_min_size(min, size))
        .filter(|candidate| !overlaps_any(*candidate, placed))
        .min_by(|a, b| {
            let da = a.min.distance(rect.min);
            let db = b.min.distance(rect.min);
            da.total_cmp(&db)
        })
        .unwrap_or_else(|| {
            let right = placed.iter().map(Rect::right).fold(f32::MIN, f32::max);
            Rect::from_min_size(Pos2::new(right, rect.min.y), size)
        })
}

fn beside(rect: Rect, other: Rect) -> [Pos2; 4] {
    let size = rect.size();
    let along = |at: f32, low: f32, high: f32, length: f32| {
        let shared = SHARED.min(length).min(high - low);
        at.clamp(low - length + shared, high - shared)
    };
    let y = along(rect.min.y, other.top(), other.bottom(), size.y);
    let x = along(rect.min.x, other.left(), other.right(), size.x);
    [
        Pos2::new(other.right(), y),
        Pos2::new(other.left() - size.x, y),
        Pos2::new(x, other.bottom()),
        Pos2::new(x, other.top() - size.y),
    ]
}

fn overlaps_any(rect: Rect, placed: &[Rect]) -> bool {
    placed.iter().any(|other| {
        let shared = rect.intersect(*other);
        shared.width() > NEAR && shared.height() > NEAR
    })
}

fn touch(a: Rect, b: Rect) -> bool {
    let shared = |low_a: f32, high_a: f32, low_b: f32, high_b: f32| {
        high_a.min(high_b) - low_a.max(low_b) > NEAR
    };
    let side_by_side = ((a.right() - b.left()).abs() < NEAR || (a.left() - b.right()).abs() < NEAR)
        && shared(a.top(), a.bottom(), b.top(), b.bottom());
    let stacked = ((a.bottom() - b.top()).abs() < NEAR || (a.top() - b.bottom()).abs() < NEAR)
        && shared(a.left(), a.right(), b.left(), b.right());
    side_by_side || stacked
}

fn bounds(rects: &[Rect]) -> Option<Rect> {
    rects.iter().copied().reduce(|a, b| a.union(b))
}

#[cfg(test)]
mod tests;
