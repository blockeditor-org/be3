use be_block_model::{Anchor, Change, Count, Document, Edit, List, Model, ObjectId};
use uuid::Uuid;

use be_block::Root;

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct Scoreboard {
    pub players: List<Player>,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct Player {
    pub name: String,
    pub score: Count,
}

impl Scoreboard {
    pub fn add(name: impl Into<String>) -> (ObjectId, Edit) {
        let player = Player {
            name: name.into(),
            score: Count::default(),
        };
        let (id, change) = Self::PLAYERS.insert(ObjectId::ROOT, Anchor::End, &player);
        (id, change.into())
    }

    pub fn score(player: ObjectId, by: i64) -> Edit {
        Player::SCORE.add(player, by).into()
    }

    pub fn rename(player: ObjectId, name: impl Into<String>) -> Edit {
        Player::NAME.set(player, &name.into()).into()
    }

    pub fn remove(player: ObjectId) -> Edit {
        Change::remove(player).into()
    }

    pub fn reset(&self) -> Edit {
        self.players
            .iter()
            .filter(|player| player.score.get() != 0)
            .map(|player| Player::SCORE.add(player.id, player.score.get().saturating_neg()))
            .collect()
    }

    pub fn standings(&self) -> Vec<ObjectId> {
        let mut players: Vec<_> = self.players.iter().collect();
        players.sort_by_key(|player| std::cmp::Reverse(player.score.get()));
        players.into_iter().map(|player| player.id).collect()
    }
}

impl Root for Scoreboard {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x0ac5_bc05_abd1_4a2a_8ce3_c740_562e_a5b2);
}

pub type ScoreboardContent = Document<Scoreboard>;
