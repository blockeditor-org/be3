use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;
use uuid::Uuid;

use crate::board::{Board, ItemId, Pile, Spot, Sprite, card_table};
use crate::cards::{Card, deck};

pub const DRAW_PILE: u32 = 0;
pub const DISCARD_PILE: u32 = 1;
const CARDS: u32 = u32::MAX - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dealt {
    pub id: ItemId,
    pub card: Card,
}

#[derive(Clone)]
pub struct Table {
    players: Vec<Uuid>,
    hands: Vec<Vec<Dealt>>,
    draw_pile: Vec<Dealt>,
    discard_pile: Vec<Dealt>,
    turn: usize,
    passes: usize,
    shuffle: ChaCha8Rng,
}

pub fn face_down(count: usize) -> Vec<(ItemId, Sprite)> {
    (0..count as u32)
        .map(|index| (ItemId::new(CARDS, index), Sprite::CardBack))
        .collect()
}

impl Table {
    pub fn deal(players: &[Uuid], hand_size: usize, decks: usize) -> Self {
        let mut shuffle = shuffle_for(players);
        let mut draw_pile: Vec<Dealt> = (0..decks)
            .flat_map(|_| deck())
            .zip(0..)
            .map(|(card, index)| Dealt {
                id: ItemId::new(CARDS, index),
                card,
            })
            .collect();
        draw_pile.shuffle(&mut shuffle);

        let mut hands = Vec::new();
        for _ in players {
            hands.push(draw_pile.split_off(draw_pile.len() - hand_size));
        }
        let face_up = draw_pile
            .pop()
            .expect("a deal leaves a card to turn face up");

        Self {
            players: players.to_vec(),
            hands,
            draw_pile,
            discard_pile: vec![face_up],
            turn: 0,
            passes: 0,
            shuffle,
        }
    }

    pub fn whose_turn(&self) -> Uuid {
        self.players[self.turn]
    }

    pub fn hand(&self) -> Vec<Card> {
        self.hands[self.turn].iter().map(|held| held.card).collect()
    }

    pub fn face_up(&self) -> Card {
        self.discard_pile
            .last()
            .expect("the deal turns a card face up")
            .card
    }

    pub fn play(&mut self, card: Card) {
        let position = self.hands[self.turn]
            .iter()
            .position(|held| held.card == card)
            .expect("a card is only played from the hand holding it");
        let played = self.hands[self.turn].remove(position);
        self.discard_pile.push(played);
        self.passes = 0;
    }

    pub fn can_draw(&self) -> bool {
        !self.draw_pile.is_empty() || self.discard_pile.len() > 1
    }

    pub fn draw(&mut self) -> Option<Card> {
        if self.draw_pile.is_empty() {
            let face_up = self
                .discard_pile
                .pop()
                .expect("the deal turns a card face up");
            self.draw_pile.append(&mut self.discard_pile);
            self.discard_pile.push(face_up);
            self.draw_pile.shuffle(&mut self.shuffle);
        }
        let drawn = self.draw_pile.pop()?;
        self.hands[self.turn].push(drawn);
        Some(drawn.card)
    }

    pub fn pass(&mut self) {
        self.passes += 1;
    }

    pub fn everyone_has_passed(&self) -> bool {
        self.passes >= self.players.len()
    }

    pub fn turn_passes_to_the_left(&mut self) {
        self.turn = (self.turn + 1) % self.players.len();
    }

    pub fn player_who_is_out(&self) -> Option<Uuid> {
        self.players
            .iter()
            .zip(&self.hands)
            .find(|(_, hand)| hand.is_empty())
            .map(|(player, _)| *player)
    }

    pub fn hand_pile(&self, player: Uuid) -> Option<u32> {
        self.players
            .iter()
            .position(|seated| *seated == player)
            .map(|seat| seat as u32 + 2)
    }

    pub fn in_hand(&self, card: Card) -> Spot {
        let position = self.hands[self.turn]
            .iter()
            .rposition(|held| held.card == card)
            .expect("only a card in the hand has a place in it");
        Spot::card(self.turn as u32 + 2, position as u32)
    }

    pub fn board(&self, viewer: Uuid) -> Board {
        let face_down = |cards: &[Dealt]| -> Vec<(ItemId, Sprite)> {
            cards
                .iter()
                .map(|held| (held.id, Sprite::CardBack))
                .collect()
        };
        let face_up = |cards: &[Dealt]| -> Vec<(ItemId, Sprite)> {
            cards
                .iter()
                .map(|held| (held.id, Sprite::Card(held.card)))
                .collect()
        };
        let opponents = self
            .players
            .iter()
            .zip(&self.hands)
            .enumerate()
            .filter(|(_, (player, _))| **player != viewer)
            .map(|(seat, (_, hand))| {
                Pile::fanned(seat as u32 + 2, format!("P{}", seat + 1), face_down(hand))
            })
            .collect();
        let middle = vec![
            Pile::stacked(DRAW_PILE, "Draw pile", face_down(&self.draw_pile)),
            Pile::stacked(DISCARD_PILE, "Discard pile", face_up(&self.discard_pile)),
        ];
        let mut board = card_table(vec![opponents, middle]);
        if let Some(seat) = self.players.iter().position(|player| *player == viewer) {
            for (card, held) in self.hands[seat].iter().enumerate() {
                board.hold(
                    held.id,
                    Sprite::Card(held.card),
                    Spot::card(seat as u32 + 2, card as u32),
                );
            }
        }
        board
    }
}

fn shuffle_for(players: &[Uuid]) -> ChaCha8Rng {
    let seed = players.iter().fold(0u128, |seed, id| seed ^ id.as_u128()) as u64;
    ChaCha8Rng::seed_from_u64(seed)
}

#[cfg(test)]
mod tests;
