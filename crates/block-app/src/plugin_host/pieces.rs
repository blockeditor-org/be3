use beui::{Rect, pos2};

use super::presenter::Piece;

pub(crate) fn uncovered(old: &[Piece], new: &[Piece]) -> Option<Vec<Rect>> {
    let mapping = |piece: &Piece| {
        let scale = |source: f32, local: f32| source / local.max(f32::EPSILON);
        let (x, y) = (
            scale(piece.source.width(), piece.local.width()),
            scale(piece.source.height(), piece.local.height()),
        );
        [
            x,
            y,
            piece.source.min.x - piece.local.min.x * x,
            piece.source.min.y - piece.local.min.y * y,
        ]
    };
    let mut mappings = old.iter().chain(new).map(mapping);
    if let Some(first) = mappings.next()
        && mappings.any(|other| {
            first
                .iter()
                .zip(other)
                .any(|(first, other)| (first - other).abs() > 1e-4)
        })
    {
        return None;
    }
    let locals = |pieces: &[Piece]| pieces.iter().map(|piece| piece.local).collect::<Vec<_>>();
    let (was, now) = (locals(old), locals(new));
    let gone = was.iter().flat_map(|piece| subtract(*piece, &now));
    let came = now.iter().flat_map(|piece| subtract(*piece, &was));
    Some(gone.chain(came).collect())
}

pub(super) fn subtract(area: Rect, holes: &[Rect]) -> Vec<Rect> {
    if !area.is_positive() {
        return Vec::new();
    }
    let covering: Vec<Rect> = holes
        .iter()
        .map(|hole| hole.intersect(area))
        .filter(|hole| hole.is_positive())
        .collect();
    if covering.is_empty() {
        return vec![area];
    }
    let mut columns = vec![area.min.x, area.max.x];
    let mut rows = vec![area.min.y, area.max.y];
    for hole in &covering {
        columns.push(hole.min.x);
        columns.push(hole.max.x);
        rows.push(hole.min.y);
        rows.push(hole.max.y);
    }
    let bounds = |values: &mut Vec<f32>, low: f32, high: f32| {
        values.retain(|value| *value > low && *value < high);
        values.push(low);
        values.push(high);
        values.sort_by(|left, right| left.total_cmp(right));
        values.dedup();
    };
    bounds(&mut columns, area.min.x, area.max.x);
    bounds(&mut rows, area.min.y, area.max.y);
    let mut pieces: Vec<Rect> = Vec::new();
    for row in rows.windows(2) {
        let mut open: Option<Rect> = None;
        for column in columns.windows(2) {
            let cell = Rect::from_min_max(pos2(column[0], row[0]), pos2(column[1], row[1]));
            if !cell.is_positive() {
                continue;
            }
            let centre = cell.center();
            if covering.iter().any(|hole| hole.contains(centre)) {
                if let Some(piece) = open.take() {
                    pieces.push(piece);
                }
                continue;
            }
            open = Some(match open {
                Some(piece) => piece.union(cell),
                None => cell,
            });
        }
        if let Some(piece) = open.take() {
            pieces.push(piece);
        }
    }
    merge_rows(pieces)
}

fn merge_rows(pieces: Vec<Rect>) -> Vec<Rect> {
    let mut merged: Vec<Rect> = Vec::new();
    for piece in pieces {
        let joined = merged.iter_mut().find(|other| {
            other.min.x == piece.min.x && other.max.x == piece.max.x && other.max.y == piece.min.y
        });
        match joined {
            Some(other) => other.max.y = piece.max.y,
            None => merged.push(piece),
        }
    }
    merged
}

#[cfg(test)]
mod tests;
