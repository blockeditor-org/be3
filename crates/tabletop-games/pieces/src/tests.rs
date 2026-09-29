use crate::checkers::MAN;
use crate::chess::{KING, KNIGHT, PAWN, ROOK};
use crate::notation::{algebraic, numbered};
use crate::{Piece, Position, Side, Square, Step};

mod a_checker_man_keeps_a_king_from_stepping_where_it_could_be_jumped;
mod a_king_cannot_castle_through_an_attacked_square;
mod a_knight_in_checkers_has_to_capture_like_everyone_else;
mod a_pawn_can_be_taken_in_passing;
mod a_pawn_reaching_the_last_rank_becomes_any_of_four_pieces;
mod checkers_jumps_are_compulsory_and_chain_to_the_end;
mod checkers_moves_are_numbered_from_the_first_players_side;
mod moves_are_written_in_algebraic_notation;
mod the_opening_position_has_the_known_move_counts;

fn square(name: &str) -> Square {
    let bytes = name.as_bytes();
    Square::new((bytes[0] - b'a') as i8, (bytes[1] - b'1') as i8)
}

fn board(men: &[(&str, &'static dyn Piece, Side)]) -> Position {
    let mut position = Position::empty(8, 8);
    for (at, piece, side) in men {
        position.place(square(at), *piece, *side);
    }
    position
}

fn written(position: &Position, side: Side) -> Vec<String> {
    let legal = position.legal(side, false);
    let mut written: Vec<String> = legal
        .iter()
        .map(|step| algebraic(position, step, &legal))
        .collect();
    written.sort();
    written
}

fn perft(position: &Position, side: Side, depth: u32) -> usize {
    if depth == 0 {
        return 1;
    }
    position
        .legal(side, false)
        .iter()
        .map(|step| {
            let mut next = position.clone();
            next.apply(step);
            perft(&next, side.other(), depth - 1)
        })
        .sum()
}

fn chess() -> Position {
    let mut position = Position::empty(8, 8);
    crate::chess::army(&mut position, Side::First);
    crate::chess::army(&mut position, Side::Second);
    position
}

fn find<'a>(steps: &'a [Step], from: &str, to: &str) -> Option<&'a Step> {
    steps
        .iter()
        .find(|step| step.from == square(from) && step.to == square(to))
}
