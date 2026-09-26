use std::cell::Cell;

use crate::{Man, Passing, Piece, Side, Square, Step};

#[derive(Clone)]
pub struct Position {
    columns: i8,
    rows: i8,
    squares: Vec<Option<Man>>,
    passing: Option<Passing>,
    probing: Cell<bool>,
}

struct Phantom;

impl Piece for Phantom {
    fn name(&self) -> &'static str {
        "phantom"
    }

    fn moves(&self, _: &Position, _: Square, _: Side, _: &mut Vec<Step>) {}
}

static PHANTOM: Phantom = Phantom;

struct Undo {
    from: Square,
    to: Square,
    original: Man,
    replaced: Option<Man>,
    captured: Vec<(Square, Man)>,
    rode: Option<(Square, Square, Man)>,
    passing: Option<Passing>,
}

impl Position {
    pub fn empty(columns: i8, rows: i8) -> Self {
        Self {
            columns,
            rows,
            squares: vec![None; columns as usize * rows as usize],
            passing: None,
            probing: Cell::new(false),
        }
    }

    pub fn columns(&self) -> i8 {
        self.columns
    }

    pub fn rows(&self) -> i8 {
        self.rows
    }

    pub fn contains(&self, square: Square) -> bool {
        (0..self.columns).contains(&square.file) && (0..self.rows).contains(&square.rank)
    }

    fn index(&self, square: Square) -> usize {
        square.rank as usize * self.columns as usize + square.file as usize
    }

    pub fn at(&self, square: Square) -> Option<Man> {
        if !self.contains(square) {
            return None;
        }
        self.squares[self.index(square)]
    }

    pub fn is_empty(&self, square: Square) -> bool {
        self.contains(square) && self.at(square).is_none()
    }

    pub fn holds_enemy(&self, square: Square, side: Side) -> bool {
        self.at(square).is_some_and(|man| man.side != side)
    }

    pub fn place(&mut self, square: Square, piece: &'static dyn Piece, side: Side) {
        let index = self.index(square);
        self.squares[index] = Some(Man {
            piece,
            side,
            moved: false,
        });
    }

    fn set(&mut self, square: Square, man: Option<Man>) {
        let index = self.index(square);
        self.squares[index] = man;
    }

    pub fn passing(&self) -> Option<Passing> {
        self.passing
    }

    pub fn probing(&self) -> bool {
        self.probing.get()
    }

    pub fn last_rank(&self, side: Side) -> i8 {
        match side {
            Side::First => self.rows - 1,
            Side::Second => 0,
        }
    }

    pub fn squares(&self) -> impl Iterator<Item = Square> + '_ {
        (0..self.rows).flat_map(move |rank| (0..self.columns).map(move |file| Square::new(file, rank)))
    }

    pub fn men(&self, side: Side) -> impl Iterator<Item = (Square, Man)> + '_ {
        let columns = self.columns as usize;
        self.squares
            .iter()
            .enumerate()
            .filter_map(move |(index, man)| {
                man.filter(|man| man.side == side).map(|man| {
                    let square = Square::new((index % columns) as i8, (index / columns) as i8);
                    (square, man)
                })
            })
    }

    pub fn steps(&self, side: Side) -> Vec<Step> {
        let mut out = Vec::new();
        for (from, man) in self.men(side) {
            man.piece.moves(self, from, side, &mut out);
        }
        out
    }

    pub fn apply(&mut self, step: &Step) {
        self.play(step);
    }

    fn play(&mut self, step: &Step) -> Option<Undo> {
        let original = self.at(step.from)?;
        let mut man = original;
        self.set(step.from, None);
        let mut captured = Vec::new();
        for square in &step.captures {
            if let Some(taken) = self.at(*square) {
                captured.push((*square, taken));
                self.set(*square, None);
            }
        }
        let mut rode = None;
        if let Some((from, to)) = step.rider
            && let Some(rider) = self.at(from)
        {
            self.set(from, None);
            rode = Some((from, to, rider));
            self.set(
                to,
                Some(Man {
                    moved: true,
                    ..rider
                }),
            );
        }
        man.moved = true;
        if let Some(piece) = step.becomes {
            man.piece = piece;
        }
        let replaced = self.at(step.to);
        self.set(step.to, Some(man));
        let passing = std::mem::replace(&mut self.passing, step.passing);
        Some(Undo {
            from: step.from,
            to: step.to,
            original,
            replaced,
            captured,
            rode,
            passing,
        })
    }

    fn undo(&mut self, undo: Undo) {
        self.passing = undo.passing;
        self.set(undo.to, undo.replaced);
        if let Some((from, to, rider)) = undo.rode {
            self.set(to, None);
            self.set(from, Some(rider));
        }
        for (square, taken) in undo.captured.into_iter().rev() {
            self.set(square, Some(taken));
        }
        self.set(undo.from, Some(undo.original));
    }

    pub fn attacked(&self, square: Square, by: Side) -> bool {
        let probing = self.probing.replace(true);
        let hit = self
            .men(by)
            .any(|(from, man)| man.piece.attacks(self, from, by, square));
        self.probing.set(probing);
        hit
    }

    pub fn threatened(&self, square: Square, by: Side) -> bool {
        if self.at(square).is_some() {
            return self.attacked(square, by);
        }
        let mut probe = self.clone();
        probe.place(square, &PHANTOM, by.other());
        probe.attacked(square, by)
    }

    pub fn royals(&self, side: Side) -> impl Iterator<Item = Square> + '_ {
        self.men(side)
            .filter(|(_, man)| man.piece.royal())
            .map(|(square, _)| square)
    }

    pub fn in_check(&self, side: Side) -> bool {
        self.royals(side)
            .any(|square| self.attacked(square, side.other()))
    }

    pub fn legal(&self, side: Side, must_capture: bool) -> Vec<Step> {
        let royals: Vec<Square> = self.royals(side).collect();
        let mut steps = self.steps(side);
        if !royals.is_empty() {
            let mut probe = self.clone();
            steps.retain(|step| {
                let Some(undo) = probe.play(step) else {
                    return false;
                };
                let safe = royals.iter().all(|royal| {
                    let royal = if *royal == step.from { step.to } else { *royal };
                    !probe.attacked(royal, side.other())
                });
                probe.undo(undo);
                safe
            });
        }
        let mut legal = steps;
        if must_capture && legal.iter().any(Step::is_capture) {
            legal.retain(Step::is_capture);
        }
        legal
    }

    pub(crate) fn key(&self, side: Side) -> Vec<u8> {
        let mut key = vec![side.index() as u8];
        for man in &self.squares {
            match man {
                None => key.push(0),
                Some(man) => {
                    key.push(1 + man.side.index() as u8 + 2 * man.moved as u8);
                    key.extend_from_slice(man.piece.name().as_bytes());
                    key.push(0);
                }
            }
        }
        if let Some(passing) = self.passing {
            key.extend_from_slice(&[passing.over.file as u8, passing.over.rank as u8]);
        }
        key
    }
}
