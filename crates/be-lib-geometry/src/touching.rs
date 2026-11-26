use crate::{Pos2, Rect, Side, Vec2};

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

/// Where along a side a touching place lines up with it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Line {
    Start,
    Middle,
    End,
}

/// A place a rect touches another: the rect, which of the placed ones it is beside, on which
/// of that one's sides, and lined up with which part of the side.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Touching {
    pub rect: Rect,
    pub beside: usize,
    pub side: Side,
    pub line: Line,
}

/// Every place a rect of `size` touches one of `placed` and overlaps none: beside each side
/// of each, lined up with the side's start, middle and end. A place two of these name is
/// listed once.
pub fn places(size: Vec2, placed: &[Rect]) -> Vec<Touching> {
    let mut places: Vec<Touching> = Vec::with_capacity(placed.len() * 12);
    for (beside, other) in placed.iter().enumerate() {
        let lined = |low: f32, high: f32, length: f32| {
            [
                (Line::Start, low),
                (Line::Middle, (low + high - length) / 2.0),
                (Line::End, high - length),
            ]
        };
        let across = lined(other.top(), other.bottom(), size.y);
        let along = lined(other.left(), other.right(), size.x);
        let sides = across
            .iter()
            .flat_map(|(line, y)| {
                [
                    (Side::Right, *line, Pos2::new(other.right(), *y)),
                    (Side::Left, *line, Pos2::new(other.left() - size.x, *y)),
                ]
            })
            .chain(along.iter().flat_map(|(line, x)| {
                [
                    (Side::Below, *line, Pos2::new(*x, other.bottom())),
                    (Side::Above, *line, Pos2::new(*x, other.top() - size.y)),
                ]
            }));
        for (side, line, min) in sides {
            let rect = Rect::from_min_size(min, size);
            if overlaps_any(rect, placed) || places.iter().any(|place| place.rect == rect) {
                continue;
            }
            places.push(Touching {
                rect,
                beside,
                side,
                line,
            });
        }
    }
    places
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
