use crate::geometry::Rect;
use crate::painter::Shape;

pub(crate) fn between(before: &[Shape], after: &[Shape]) -> Rect {
    let shared = before
        .iter()
        .zip(after)
        .take_while(|(before, after)| before == after)
        .count();
    let remaining = before.len().min(after.len()) - shared;
    let matching = (1..=remaining)
        .take_while(|offset| before[before.len() - offset] == after[after.len() - offset])
        .count();
    let changed = before[shared..before.len() - matching]
        .iter()
        .chain(&after[shared..after.len() - matching]);
    changed.fold(Rect::NOTHING, |region, shape| region.union(bounds(shape)))
}

fn bounds(shape: &Shape) -> Rect {
    match shape {
        Shape::Rect {
            rect,
            stroke_width,
            clip,
            ..
        } => rect.expand(*stroke_width).intersect(*clip),
        Shape::Text {
            origin,
            galley,
            clip,
            ..
        } => Rect::from_min_size(*origin, galley.size()).intersect(*clip),
    }
}

#[cfg(test)]
mod tests;
