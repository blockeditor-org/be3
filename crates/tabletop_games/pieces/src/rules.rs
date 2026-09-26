use std::convert::Infallible;

use game_api::board::{Grid, Shade, Sprite, Tint};
use game_api::{GameHelper, GameScreen, Move, Scene, Spot};
use uuid::Uuid;

use crate::checkers::dark;
use crate::notation::{algebraic, numbered};
use crate::{Position, Side, Square, Step};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notation {
    Algebraic,
    Numbered,
}

pub struct Army {
    pub name: &'static str,
    pub color: u8,
    pub must_capture: bool,
}

pub struct Rules {
    pub columns: i8,
    pub rows: i8,
    pub setup: fn(&mut Position),
    pub armies: [Army; 2],
    pub notation: Notation,
    pub quiet_limit: usize,
}

impl Rules {
    pub fn start(&self) -> Position {
        let mut position = Position::empty(self.columns, self.rows);
        (self.setup)(&mut position);
        position
    }

    pub fn army(&self, side: Side) -> &Army {
        &self.armies[side.index()]
    }

    pub fn label(&self, position: &Position, step: &Step, legal: &[Step]) -> String {
        match self.notation {
            Notation::Algebraic => algebraic(position, step, legal),
            Notation::Numbered => numbered(position, step),
        }
    }
}

enum Ending {
    Won { winner: Side, how: &'static str },
    Resigned(Side),
    Drawn(&'static str),
}

pub fn play(helper: GameHelper<'_>, rules: &Rules) -> Result<Infallible, GameScreen> {
    let mut position = rules.start();
    let mut side = Side::First;
    let mut seats: [Option<Uuid>; 2] = [None, None];
    let mut last: Option<Step> = None;
    let mut quiet = 0;
    let mut seen = vec![position.key(side)];

    loop {
        let legal = position.legal(side, rules.army(side).must_capture);
        let check = position.in_check(side);
        if check && rules.notation == Notation::Algebraic {
            helper.annotate(if legal.is_empty() { "#" } else { "+" });
        }
        let repeated = seen.iter().filter(|key| seen.last() == Some(key)).count();
        let ending = if legal.is_empty() {
            if check || position.royals(side).next().is_none() {
                Some(Ending::Won {
                    winner: side.other(),
                    how: if check {
                        "checkmate"
                    } else {
                        "leaving no move"
                    },
                })
            } else {
                Some(Ending::Drawn("stalemate"))
            }
        } else if quiet >= rules.quiet_limit {
            Some(Ending::Drawn("too many moves without progress"))
        } else if repeated >= 3 {
            Some(Ending::Drawn("threefold repetition"))
        } else {
            None
        };

        let table = Table {
            rules,
            position: &position,
            seats,
            last: last.as_ref(),
            check: check.then_some(side),
        };
        if let Some(ending) = ending {
            return helper.game_over(|viewer| {
                Scene::new(table.ending(&ending, viewer)).on(table.grid(viewer))
            });
        }

        let can_move = |player: Uuid| match seats[side.index()] {
            Some(seated) => seated == player,
            None => seats[side.other().index()] != Some(player),
        };
        let mut chosen = None;
        let mut resigned = None;
        helper.action(
            |viewer| {
                Scene::new(table.status(side, viewer, can_move(viewer))).on(table.grid(viewer))
            },
            |player, choose| {
                if can_move(player) {
                    let flipped = table.flipped(player);
                    let listing = helper.listing();
                    for (index, step) in legal.iter().enumerate() {
                        let label = match listing {
                            true => rules.label(&position, step, &legal),
                            false => String::new(),
                        };
                        let gesture = Move::new(label)
                            .drag(table.spot(step.from, flipped), table.spot(step.to, flipped));
                        if choose(gesture) {
                            chosen = Some((index, player));
                            return;
                        }
                    }
                }
                if seats.contains(&Some(player)) && choose(Move::new("Resign").recorded("Resigned"))
                {
                    resigned = Some(player);
                }
            },
        )?;

        if let Some(player) = resigned {
            let loser = match seats[0] == Some(player) {
                true => Side::First,
                false => Side::Second,
            };
            let ending = Ending::Resigned(loser);
            return helper.game_over(|viewer| {
                Scene::new(table.ending(&ending, viewer)).on(table.grid(viewer))
            });
        }
        let Some((index, player)) = chosen else {
            continue;
        };
        helper.describe_last_turn(rules.label(&position, &legal[index], &legal));
        seats[side.index()].get_or_insert(player);
        let step = legal
            .into_iter()
            .nth(index)
            .expect("the chosen move is legal");
        let irreversible = step.is_capture()
            || position
                .at(step.from)
                .is_some_and(|man| man.piece.irreversible());
        position.apply(&step);
        side = side.other();
        if irreversible {
            quiet = 0;
            seen.clear();
        } else {
            quiet += 1;
        }
        seen.push(position.key(side));
        last = Some(step);
    }
}

struct Table<'a> {
    rules: &'a Rules,
    position: &'a Position,
    seats: [Option<Uuid>; 2],
    last: Option<&'a Step>,
    check: Option<Side>,
}

impl Table<'_> {
    fn side_of(&self, viewer: Uuid) -> Option<Side> {
        [Side::First, Side::Second]
            .into_iter()
            .find(|side| self.seats[side.index()] == Some(viewer))
    }

    fn flipped(&self, viewer: Uuid) -> bool {
        match self.side_of(viewer) {
            Some(side) => side == Side::Second,
            None => self.seats[0].is_some() && self.seats[1].is_none(),
        }
    }

    fn spot(&self, square: Square, flipped: bool) -> Spot {
        let columns = self.position.columns() - 1;
        let rows = self.position.rows() - 1;
        match flipped {
            false => Spot::tile(square.file as u32, (rows - square.rank) as u32),
            true => Spot::tile((columns - square.file) as u32, square.rank as u32),
        }
    }

    fn status(&self, side: Side, viewer: Uuid, can_move: bool) -> String {
        let army = self.rules.army(side).name;
        if can_move {
            match self.check == Some(side) {
                true => format!("Your move ({army}) - you are in check"),
                false => format!("Your move ({army})"),
            }
        } else if self.side_of(viewer).is_some() {
            format!("Waiting for {army}...")
        } else {
            format!("{army} to move")
        }
    }

    fn ending(&self, ending: &Ending, viewer: Uuid) -> String {
        let name = |side: Side| self.rules.army(side).name;
        let mine = self.side_of(viewer);
        match *ending {
            Ending::Won { winner, how } if mine == Some(winner) => format!("You win by {how}"),
            Ending::Won { winner, how } if mine.is_some() => {
                format!("You lose - {} wins by {how}", name(winner))
            }
            Ending::Won { winner, how } => format!("{} wins by {how}", name(winner)),
            Ending::Resigned(loser) if mine == Some(loser) => "You resigned".to_owned(),
            Ending::Resigned(loser) => {
                format!("{} resigned - {} wins", name(loser), name(loser.other()))
            }
            Ending::Drawn(reason) => format!("Draw by {reason}"),
        }
    }

    fn grid(&self, viewer: Uuid) -> Grid {
        let flipped = self.flipped(viewer);
        let mut grid = Grid::new(self.position.columns() as u32, self.position.rows() as u32);
        let marked: Vec<Square> = self
            .last
            .map(|step| {
                let mut marked = vec![step.from, step.to];
                marked.extend(&step.via);
                marked
            })
            .unwrap_or_default();
        let endangered: Vec<Square> = self
            .check
            .map(|side| self.position.royals(side).collect())
            .unwrap_or_default();
        for square in self.position.squares() {
            let Spot::Tile { column, row } = self.spot(square, flipped) else {
                continue;
            };
            let shade = match dark(square) {
                true => Shade::Dark,
                false => Shade::Light,
            };
            grid.place(column, row, Sprite::Square(shade));
            if marked.contains(&square) {
                grid.place(column, row, Sprite::Tint(Tint::LastMove));
            }
            if endangered.contains(&square) {
                grid.place(column, row, Sprite::Tint(Tint::Danger));
            }
            if let Some(man) = self.position.at(square) {
                let color = self.rules.army(man.side).color;
                grid.place(column, row, Sprite::piece(man.piece.name(), color));
            }
        }
        grid
    }
}
