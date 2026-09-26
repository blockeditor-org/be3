use crate::movement::{ADJACENT, DIAGONAL, ORTHOGONAL, leap, leaps_to};
use crate::{Leaper, Passing, Piece, Position, Rider, Side, Square, Step};

const ALL_LINES: [(i8, i8); 8] = ADJACENT;
const KNIGHT_JUMPS: [(i8, i8); 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (1, -2),
    (-1, -2),
    (-2, -1),
    (-2, 1),
    (-1, 2),
];

pub static KING: King = King;
pub static QUEEN: Rider = Rider {
    name: "queen",
    letter: "Q",
    directions: &ALL_LINES,
};
pub static ROOK: Rider = Rider {
    name: "rook",
    letter: "R",
    directions: &ORTHOGONAL,
};
pub static BISHOP: Rider = Rider {
    name: "bishop",
    letter: "B",
    directions: &DIAGONAL,
};
pub static KNIGHT: Leaper = Leaper {
    name: "knight",
    letter: "N",
    offsets: &KNIGHT_JUMPS,
};
pub static PAWN: Pawn = Pawn {
    promotions: &[&QUEEN, &ROOK, &BISHOP, &KNIGHT],
};

pub static BACK_RANK: [&dyn Piece; 8] = [
    &ROOK, &KNIGHT, &BISHOP, &QUEEN, &KING, &BISHOP, &KNIGHT, &ROOK,
];

pub fn army(position: &mut Position, side: Side) {
    let back = match side {
        Side::First => 0,
        Side::Second => position.rows() - 1,
    };
    for (file, piece) in BACK_RANK.into_iter().enumerate() {
        position.place(Square::new(file as i8, back), piece, side);
        position.place(Square::new(file as i8, back + side.forward()), &PAWN, side);
    }
}

pub struct King;

impl Piece for King {
    fn name(&self) -> &'static str {
        "king"
    }

    fn letter(&self) -> &'static str {
        "K"
    }

    fn royal(&self) -> bool {
        true
    }

    fn attacks(&self, position: &Position, from: Square, side: Side, target: Square) -> bool {
        position.holds_enemy(target, side) && leaps_to(from, &ADJACENT, target)
    }

    fn moves(&self, position: &Position, from: Square, side: Side, out: &mut Vec<Step>) {
        leap(position, from, side, &ADJACENT, out);
        if position.probing() || position.at(from).is_none_or(|king| king.moved) {
            return;
        }
        let enemy = side.other();
        if position.attacked(from, enemy) {
            return;
        }
        for direction in [-1, 1] {
            let mut rook = from.offset(direction, 0);
            while position.is_empty(rook) {
                rook = rook.offset(direction, 0);
            }
            let Some(man) = position.at(rook) else {
                continue;
            };
            if man.side != side || man.moved || man.piece.name() != ROOK.name {
                continue;
            }
            if (rook.file - from.file).abs() < 3 {
                continue;
            }
            let crossed = from.offset(direction, 0);
            let landing = from.offset(2 * direction, 0);
            if position.threatened(crossed, enemy) || position.threatened(landing, enemy) {
                continue;
            }
            out.push(Step::new(from, landing).with_rider(rook, crossed));
        }
    }
}

pub struct Pawn {
    pub promotions: &'static [&'static dyn Piece],
}

impl Pawn {
    fn arrive(&self, position: &Position, side: Side, step: Step, out: &mut Vec<Step>) {
        if step.to.rank != position.last_rank(side) || self.promotions.is_empty() {
            out.push(step);
            return;
        }
        for piece in self.promotions {
            out.push(step.clone().becoming(*piece));
        }
    }
}

impl Piece for Pawn {
    fn name(&self) -> &'static str {
        "pawn"
    }

    fn irreversible(&self) -> bool {
        true
    }

    fn attacks(&self, position: &Position, from: Square, side: Side, target: Square) -> bool {
        position.holds_enemy(target, side)
            && target.rank - from.rank == side.forward()
            && (target.file - from.file).abs() == 1
    }

    fn moves(&self, position: &Position, from: Square, side: Side, out: &mut Vec<Step>) {
        let forward = side.forward();
        let one = from.offset(0, forward);
        if position.is_empty(one) {
            self.arrive(position, side, Step::new(from, one), out);
            let two = one.offset(0, forward);
            let fresh = position.at(from).is_some_and(|pawn| !pawn.moved);
            if fresh && position.is_empty(two) && two.rank != position.last_rank(side) {
                out.push(Step::new(from, two).opening(Passing {
                    over: one,
                    taken: two,
                }));
            }
        }
        for file in [-1, 1] {
            let to = from.offset(file, forward);
            if position.holds_enemy(to, side) {
                self.arrive(position, side, Step::new(from, to).capturing(to), out);
            } else if let Some(passing) = position.passing()
                && passing.over == to
                && position.holds_enemy(passing.taken, side)
            {
                out.push(Step::new(from, to).capturing(passing.taken));
            }
        }
    }
}
