use beui::Rect;

use super::presenter::Piece;

pub fn uncovered(old: &[Piece], new: &[Piece]) -> Option<Vec<Rect>> {
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
    let gone = was.iter().flat_map(|piece| piece.subtract(&now));
    let came = now.iter().flat_map(|piece| piece.subtract(&was));
    Some(gone.chain(came).collect())
}

#[cfg(test)]
mod tests;
