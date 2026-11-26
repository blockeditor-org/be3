//! What the hunt's host and its players say on the block's host channel
//! (guides/blocks/hosted.md, "The channel"): a player asks to dig, and the host tells each
//! player the world as that player may see it.

use be_type_gem_hunt::Hunt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// What a player asks of the host.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Ask {
    Dig(u16),
}

/// The world as one player sees it: only the cells dug, never where a gem still lies.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Board {
    pub dug: Vec<Dug>,
    /// The gems the world hides, found or not.
    pub gems: u16,
    /// The gems dug up so far.
    pub found: u16,
    /// The gems this player dug up.
    pub yours: u16,
}

/// A dug cell and what it showed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Dug {
    pub cell: u16,
    pub found: Found,
}

/// What digging a cell showed: a gem, or how many gems lie around it. A gem's cell says no
/// more, since the count around it would tell where other gems lie.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Found {
    Gem,
    Near(u8),
}

impl Board {
    /// What `player` may see of `hunt`.
    pub fn of(hunt: &Hunt, player: Uuid) -> Self {
        Self {
            dug: hunt
                .dug
                .keys()
                .map(|&cell| Dug {
                    cell,
                    found: match hunt.has_gem(cell) {
                        true => Found::Gem,
                        false => Found::Near(hunt.near(cell)),
                    },
                })
                .collect(),
            gems: hunt.gems.len() as u16,
            found: hunt.found() as u16,
            yours: hunt.found_by(player) as u16,
        }
    }

    pub fn dug(&self, cell: u16) -> Option<Dug> {
        self.dug.iter().copied().find(|dug| dug.cell == cell)
    }

    pub fn over(&self) -> bool {
        self.gems > 0 && self.found == self.gems
    }
}

pub fn encode<T: Serialize>(message: &T) -> Vec<u8> {
    serde_json::to_vec(message).expect("a channel message encodes")
}

pub fn decode<T: for<'de> Deserialize<'de>>(payload: &[u8]) -> Option<T> {
    serde_json::from_slice(payload).ok()
}
