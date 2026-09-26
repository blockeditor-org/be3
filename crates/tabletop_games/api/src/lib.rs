use std::cell::{Cell, RefCell};
use std::convert::Infallible;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use board::{Board, Control, Gesture, Move, Scene, Spot};

pub mod board;
pub mod build;
pub mod cards;
pub mod guest;
pub mod table;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct GameAction {
    pub actor: Uuid,
    pub action: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct GameScreen {
    pub description: String,
    pub board: Box<Board>,
    pub actions: Vec<GameActionOption>,
    pub history: Vec<Turn>,
    pub columns: Box<[String]>,
    pub ending: Option<Ending>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Ending {
    pub score: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Turn {
    pub actor: Uuid,
    pub entry: u32,
    pub description: String,
    pub column: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct GameActionOption {
    pub label: String,
    pub gesture: Option<Gesture>,
    pub effect: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum Command {
    Play(GameAction),
    Show(Uuid),
}

pub type Choose<'a> = dyn FnMut(Move) -> bool + 'a;

enum Source<'a> {
    Log {
        actions: &'a [GameAction],
        cursor: Cell<usize>,
        player: Uuid,
    },
    Host {
        played: Cell<u32>,
    },
}

pub struct GameHelper<'a> {
    source: Source<'a>,
    history: RefCell<Vec<Turn>>,
    columns: RefCell<Vec<String>>,
    listing: Cell<bool>,
}

impl<'a> GameHelper<'a> {
    pub fn new(actions: &'a [GameAction], player: Uuid) -> Self {
        Self::from(Source::Log {
            actions,
            cursor: Cell::new(0),
            player,
        })
    }

    pub fn hosted() -> GameHelper<'static> {
        GameHelper::from(Source::Host {
            played: Cell::new(0),
        })
    }

    fn from(source: Source<'a>) -> Self {
        Self {
            source,
            history: RefCell::new(Vec::new()),
            columns: RefCell::new(Vec::new()),
            listing: Cell::new(false),
        }
    }

    pub fn columns<Name: Into<String>>(&self, names: impl IntoIterator<Item = Name>) {
        *self.columns.borrow_mut() = names.into_iter().map(Into::into).collect();
    }

    pub fn listing(&self) -> bool {
        self.listing.get()
    }

    pub fn describe_last_turn(&self, description: String) {
        if let Some(last) = self.history.borrow_mut().last_mut() {
            last.description = description;
        }
    }

    pub fn annotate(&self, suffix: &str) {
        if let Some(last) = self.history.borrow_mut().last_mut() {
            last.description.push_str(suffix);
        }
    }

    fn screen(&self, scene: Scene, actions: Vec<GameActionOption>, over: bool) -> GameScreen {
        GameScreen {
            description: scene.description,
            board: Box::new(scene.board),
            actions,
            history: self.history.borrow().clone(),
            columns: self.columns.borrow().clone().into_boxed_slice(),
            ending: over.then_some(Ending { score: scene.score }),
        }
    }

    pub fn action<S: Into<Scene>>(
        &self,
        describe: impl Fn(Uuid) -> S,
        mut body: impl FnMut(Uuid, &mut Choose<'_>),
    ) -> Result<(), GameScreen> {
        match &self.source {
            Source::Log {
                actions,
                cursor,
                player,
            } => {
                while let Some(entry) = actions.get(cursor.get()) {
                    let index = cursor.get();
                    cursor.set(index + 1);
                    if self.replay(entry, index as u32, &mut body) {
                        return Ok(());
                    }
                }
                Err(self.list(*player, &describe, &mut body))
            }
            Source::Host { played } => loop {
                match guest::next() {
                    Command::Play(entry) => {
                        let index = played.get();
                        played.set(index + 1);
                        if self.replay(&entry, index, &mut body) {
                            return Ok(());
                        }
                    }
                    Command::Show(player) => {
                        guest::present(&self.list(player, &describe, &mut body))
                    }
                }
            },
        }
    }

    fn replay(
        &self,
        entry: &GameAction,
        index: u32,
        body: &mut impl FnMut(Uuid, &mut Choose<'_>),
    ) -> bool {
        let Ok(target) = bincode::deserialize::<u32>(&entry.action) else {
            return false;
        };
        let mut seen = 0u32;
        let mut matched = None;
        body(entry.actor, &mut |offered: Move| {
            let is_target = matched.is_none() && seen == target;
            seen += 1;
            if is_target {
                matched = Some((offered.recorded.unwrap_or(offered.label), offered.column));
            }
            is_target
        });
        let Some((description, column)) = matched else {
            return false;
        };
        self.history.borrow_mut().push(Turn {
            actor: entry.actor,
            entry: index,
            description,
            column,
        });
        true
    }

    fn list<S: Into<Scene>>(
        &self,
        player: Uuid,
        describe: &impl Fn(Uuid) -> S,
        body: &mut impl FnMut(Uuid, &mut Choose<'_>),
    ) -> GameScreen {
        let mut index = 0u32;
        let mut actions = Vec::new();
        self.listing.set(true);
        body(player, &mut |offered: Move| {
            actions.push(GameActionOption {
                label: offered.label,
                gesture: offered.gesture,
                effect: bincode::serialize(&index).expect("index encoding is infallible"),
            });
            index += 1;
            false
        });
        self.listing.set(false);
        self.screen(describe(player).into(), actions, false)
    }

    pub fn turn(
        &self,
        whose: Uuid,
        yours: &str,
        theirs: &str,
        board: impl Fn(Uuid) -> Board,
        mut choices: impl FnMut(&mut Choose<'_>),
    ) -> Result<(), GameScreen> {
        self.action(
            |player| Scene::new(if player == whose { yours } else { theirs }).on(board(player)),
            |player, choose| {
                if player == whose {
                    choices(choose);
                }
            },
        )
    }

    pub fn gather(
        &self,
        minimum: usize,
        seat: Spot,
        board: impl Fn(&[Uuid], Uuid) -> Board,
    ) -> Result<Vec<Uuid>, GameScreen> {
        let mut players: Vec<Uuid> = Vec::new();
        loop {
            let joined = players.clone();
            let mut started = false;
            self.action(
                |player| {
                    let description = if !joined.contains(&player) {
                        "Join the game".to_owned()
                    } else if joined.len() < minimum {
                        "Waiting for another player to join...".to_owned()
                    } else {
                        format!("{} players joined - start when ready", joined.len())
                    };
                    Scene::new(description).on(board(&joined, player))
                },
                |player, choose| {
                    if !players.contains(&player) {
                        if choose(Move::new("Join the game").click(seat).recorded("joins")) {
                            players.push(player);
                        }
                    } else if players.len() >= minimum
                        && choose(Move::new("Start the game").click(seat).recorded("deals"))
                    {
                        started = true;
                    }
                },
            )?;
            if started {
                return Ok(players);
            }
        }
    }

    pub fn game_over<S: Into<Scene>>(
        &self,
        describe: impl Fn(Uuid) -> S,
    ) -> Result<Infallible, GameScreen> {
        match &self.source {
            Source::Log { player, .. } => {
                Err(self.screen(describe(*player).into(), Vec::new(), true))
            }
            Source::Host { .. } => loop {
                if let Command::Show(player) = guest::next() {
                    guest::present(&self.screen(describe(player).into(), Vec::new(), true));
                }
            },
        }
    }
}

#[cfg(test)]
mod tests;
