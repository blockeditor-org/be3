use std::cell::Cell;
use std::convert::Infallible;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use board::{Board, Gesture, Move, Scene, Spot};

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
    pub board: Board,
    pub actions: Vec<GameActionOption>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct GameActionOption {
    pub label: String,
    pub gesture: Option<Gesture>,
    pub effect: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct GameRequest {
    pub actions: Vec<GameAction>,
    pub player: Uuid,
}

pub type Choose<'a> = dyn FnMut(Move) -> bool + 'a;

pub struct GameHelper<'a> {
    actions: &'a [GameAction],
    cursor: Cell<usize>,
    player: Uuid,
}

impl<'a> GameHelper<'a> {
    pub fn new(actions: &'a [GameAction], player: Uuid) -> Self {
        Self {
            actions,
            cursor: Cell::new(0),
            player,
        }
    }

    pub fn action<S: Into<Scene>>(
        &self,
        describe: impl Fn(Uuid) -> S,
        mut body: impl FnMut(Uuid, &mut Choose<'_>),
    ) -> Result<(), GameScreen> {
        while let Some(entry) = self.actions.get(self.cursor.get()) {
            self.cursor.set(self.cursor.get() + 1);
            let Ok(target) = bincode::deserialize::<u32>(&entry.action) else {
                continue;
            };
            let mut seen = 0u32;
            let mut matched = false;
            body(entry.actor, &mut |_offered: Move| {
                let is_target = !matched && seen == target;
                seen += 1;
                matched |= is_target;
                is_target
            });
            if matched {
                return Ok(());
            }
        }
        let mut index = 0u32;
        let mut actions = Vec::new();
        body(self.player, &mut |offered: Move| {
            actions.push(GameActionOption {
                label: offered.label,
                gesture: offered.gesture,
                effect: bincode::serialize(&index).expect("index encoding is infallible"),
            });
            index += 1;
            false
        });
        let scene = describe(self.player).into();
        Err(GameScreen {
            description: scene.description,
            board: scene.board,
            actions,
        })
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

    pub fn gather(&self, minimum: usize) -> Result<Vec<Uuid>, GameScreen> {
        let mut players: Vec<Uuid> = Vec::new();
        loop {
            let joined = players.clone();
            let mut started = false;
            self.action(
                move |player| {
                    if !joined.contains(&player) {
                        "Join the game".to_owned()
                    } else if joined.len() < minimum {
                        "Waiting for another player to join...".to_owned()
                    } else {
                        format!("{} players joined - start when ready", joined.len())
                    }
                },
                |player, choose| {
                    if !players.contains(&player) {
                        if choose(Move::new("Join the game")) {
                            players.push(player);
                        }
                    } else if players.len() >= minimum && choose(Move::new("Start the game")) {
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
        let scene = describe(self.player).into();
        Err(GameScreen {
            description: scene.description,
            board: scene.board,
            actions: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests;
