use crate::movement::DIAGONAL;
use crate::{Piece, Position, Side, Square, Step};

pub static MAN: CheckerMan = CheckerMan {
    crowned: &CROWNED,
};
pub static CROWNED: CheckerKing = CheckerKing;

pub fn army(position: &mut Position, side: Side, ranks: i8) {
    for rank in 0..ranks {
        let rank = match side {
            Side::First => rank,
            Side::Second => position.rows() - 1 - rank,
        };
        for file in 0..position.columns() {
            if dark(Square::new(file, rank)) {
                position.place(Square::new(file, rank), &MAN, side);
            }
        }
    }
}

pub fn dark(square: Square) -> bool {
    (square.file + square.rank) % 2 == 0
}

pub struct CheckerMan {
    pub crowned: &'static dyn Piece,
}

impl Piece for CheckerMan {
    fn name(&self) -> &'static str {
        "man"
    }

    fn irreversible(&self) -> bool {
        true
    }

    fn moves(&self, position: &Position, from: Square, side: Side, out: &mut Vec<Step>) {
        let forward = side.forward();
        diagonals(
            position,
            from,
            side,
            &[(-1, forward), (1, forward)],
            Some(self.crowned),
            out,
        );
    }
}

pub struct CheckerKing;

impl Piece for CheckerKing {
    fn name(&self) -> &'static str {
        "crowned"
    }

    fn letter(&self) -> &'static str {
        "C"
    }

    fn moves(&self, position: &Position, from: Square, side: Side, out: &mut Vec<Step>) {
        diagonals(position, from, side, &DIAGONAL, None, out);
    }
}

pub fn diagonals(
    position: &Position,
    from: Square,
    side: Side,
    directions: &[(i8, i8)],
    crown: Option<&'static dyn Piece>,
    out: &mut Vec<Step>,
) {
    let last = position.last_rank(side);
    for &(file, rank) in directions {
        let to = from.offset(file, rank);
        if position.is_empty(to) {
            let step = Step::new(from, to);
            out.push(match crown {
                Some(crown) if to.rank == last => step.becoming(crown),
                _ => step,
            });
        }
    }
    let mut chain = Chain {
        position,
        start: from,
        side,
        directions,
        crown,
        landings: Vec::new(),
        taken: Vec::new(),
    };
    chain.jump(from, out);
}

struct Chain<'a> {
    position: &'a Position,
    start: Square,
    side: Side,
    directions: &'a [(i8, i8)],
    crown: Option<&'static dyn Piece>,
    landings: Vec<Square>,
    taken: Vec<Square>,
}

impl Chain<'_> {
    fn jump(&mut self, at: Square, out: &mut Vec<Step>) {
        let mut extended = false;
        for &(file, rank) in self.directions {
            let over = at.offset(file, rank);
            let landing = over.offset(file, rank);
            if !self.position.holds_enemy(over, self.side) || self.taken.contains(&over) {
                continue;
            }
            if !(self.position.is_empty(landing) || landing == self.start) {
                continue;
            }
            extended = true;
            self.taken.push(over);
            self.landings.push(landing);
            let crowned = self
                .crown
                .filter(|_| landing.rank == self.position.last_rank(self.side));
            match crowned {
                Some(crown) => out.push(self.step().becoming(crown)),
                None => self.jump(landing, out),
            }
            self.taken.pop();
            self.landings.pop();
        }
        if !extended && !self.taken.is_empty() {
            out.push(self.step());
        }
    }

    fn step(&self) -> Step {
        let (to, via) = self
            .landings
            .split_last()
            .expect("a jump lands somewhere");
        let mut step = Step::new(self.start, *to);
        step.via = via.to_vec();
        step.captures = self.taken.clone();
        step
    }
}
