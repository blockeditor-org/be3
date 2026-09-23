use super::*;

fn rect(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> beui::Rect {
    beui::Rect::from_min_max(beui::pos2(min_x, min_y), beui::pos2(max_x, max_y))
}

fn area(pieces: &[beui::Rect]) -> f32 {
    pieces
        .iter()
        .map(|piece| piece.width() * piece.height())
        .sum()
}

fn disjoint(pieces: &[beui::Rect]) -> bool {
    pieces.iter().enumerate().all(|(index, piece)| {
        pieces[index + 1..]
            .iter()
            .all(|other| !piece.intersect(*other).is_positive())
    })
}

mod a_hole_in_the_middle_leaves_a_ring;
mod nothing_is_left_when_a_hole_covers_everything;
mod pieces_of_two_holes_stay_disjoint;
