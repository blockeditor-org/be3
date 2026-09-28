use std::fmt;

pub use movement::{Leaper, Rider, leap, slide};
pub use position::Position;
pub use rules::{Army, Notation, Rules, play};

pub mod checkers;
pub mod chess;
mod movement;
mod notation;
mod position;
mod rules;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Square {
    pub file: i8,
    pub rank: i8,
}

impl Square {
    pub const fn new(file: i8, rank: i8) -> Self {
        Self { file, rank }
    }

    pub fn offset(self, file: i8, rank: i8) -> Self {
        Self::new(self.file + file, self.rank + rank)
    }
}

impl fmt::Display for Square {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}{}",
            (b'a' + self.file as u8) as char,
            self.rank + 1
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    First,
    Second,
}

impl Side {
    pub fn other(self) -> Self {
        match self {
            Side::First => Side::Second,
            Side::Second => Side::First,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Side::First => 0,
            Side::Second => 1,
        }
    }

    pub fn forward(self) -> i8 {
        match self {
            Side::First => 1,
            Side::Second => -1,
        }
    }
}

pub trait Piece: Sync {
    fn name(&self) -> &'static str;

    fn letter(&self) -> &'static str {
        ""
    }

    fn royal(&self) -> bool {
        false
    }

    fn irreversible(&self) -> bool {
        false
    }

    fn moves(&self, position: &Position, from: Square, side: Side, out: &mut Vec<Step>);

    fn attacks(&self, position: &Position, from: Square, side: Side, target: Square) -> bool {
        let mut out = Vec::new();
        self.moves(position, from, side, &mut out);
        out.iter().any(|step| step.captures.contains(&target))
    }
}

impl PartialEq for dyn Piece {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}

impl fmt::Debug for dyn Piece {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Man {
    pub piece: &'static dyn Piece,
    pub side: Side,
    pub moved: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Passing {
    pub over: Square,
    pub taken: Square,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub from: Square,
    pub to: Square,
    pub via: Vec<Square>,
    pub captures: Vec<Square>,
    pub rider: Option<(Square, Square)>,
    pub becomes: Option<&'static dyn Piece>,
    pub passing: Option<Passing>,
}

impl Step {
    pub fn new(from: Square, to: Square) -> Self {
        Self {
            from,
            to,
            via: Vec::new(),
            captures: Vec::new(),
            rider: None,
            becomes: None,
            passing: None,
        }
    }

    pub fn capturing(mut self, square: Square) -> Self {
        self.captures.push(square);
        self
    }

    pub fn becoming(mut self, piece: &'static dyn Piece) -> Self {
        self.becomes = Some(piece);
        self
    }

    pub fn with_rider(mut self, from: Square, to: Square) -> Self {
        self.rider = Some((from, to));
        self
    }

    pub fn opening(mut self, passing: Passing) -> Self {
        self.passing = Some(passing);
        self
    }

    pub fn is_capture(&self) -> bool {
        !self.captures.is_empty()
    }
}

#[cfg(test)]
mod tests;
