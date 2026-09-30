use crate::{Piece, Position, Side, Square, Step};

pub const ORTHOGONAL: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
pub const DIAGONAL: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
pub const ADJACENT: [(i8, i8); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

pub fn slide(
    position: &Position,
    from: Square,
    side: Side,
    directions: &[(i8, i8)],
    out: &mut Vec<Step>,
) {
    for &(file, rank) in directions {
        let mut to = from.offset(file, rank);
        while position.contains(to) {
            match position.at(to) {
                None => out.push(Step::new(from, to)),
                Some(man) => {
                    if man.side != side {
                        out.push(Step::new(from, to).capturing(to));
                    }
                    break;
                }
            }
            to = to.offset(file, rank);
        }
    }
}

pub fn leap(
    position: &Position,
    from: Square,
    side: Side,
    offsets: &[(i8, i8)],
    out: &mut Vec<Step>,
) {
    for &(file, rank) in offsets {
        let to = from.offset(file, rank);
        if !position.contains(to) {
            continue;
        }
        match position.at(to) {
            None => out.push(Step::new(from, to)),
            Some(man) if man.side != side => out.push(Step::new(from, to).capturing(to)),
            Some(_) => {}
        }
    }
}

pub fn slides_to(
    position: &Position,
    from: Square,
    directions: &[(i8, i8)],
    target: Square,
) -> bool {
    let (file, rank) = (target.file - from.file, target.rank - from.rank);
    let distance = file.abs().max(rank.abs());
    if distance == 0 {
        return false;
    }
    let (file, rank) = (file / distance, rank / distance);
    if from.offset(file * distance, rank * distance) != target
        || !directions.contains(&(file, rank))
    {
        return false;
    }
    (1..distance).all(|step| position.at(from.offset(file * step, rank * step)).is_none())
}

pub fn leaps_to(from: Square, offsets: &[(i8, i8)], target: Square) -> bool {
    offsets.contains(&(target.file - from.file, target.rank - from.rank))
}

fn enemy(position: &Position, side: Side, target: Square) -> bool {
    position.holds_enemy(target, side)
}

pub struct Rider {
    pub name: &'static str,
    pub letter: &'static str,
    pub directions: &'static [(i8, i8)],
}

impl Piece for Rider {
    fn name(&self) -> &'static str {
        self.name
    }

    fn letter(&self) -> &'static str {
        self.letter
    }

    fn moves(&self, position: &Position, from: Square, side: Side, out: &mut Vec<Step>) {
        slide(position, from, side, self.directions, out);
    }

    fn attacks(&self, position: &Position, from: Square, side: Side, target: Square) -> bool {
        enemy(position, side, target) && slides_to(position, from, self.directions, target)
    }
}

pub struct Leaper {
    pub name: &'static str,
    pub letter: &'static str,
    pub offsets: &'static [(i8, i8)],
}

impl Piece for Leaper {
    fn name(&self) -> &'static str {
        self.name
    }

    fn letter(&self) -> &'static str {
        self.letter
    }

    fn moves(&self, position: &Position, from: Square, side: Side, out: &mut Vec<Step>) {
        leap(position, from, side, self.offsets, out);
    }

    fn attacks(&self, position: &Position, from: Square, side: Side, target: Square) -> bool {
        enemy(position, side, target) && leaps_to(from, self.offsets, target)
    }
}
