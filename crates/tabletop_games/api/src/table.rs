use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;
use uuid::Uuid;

use crate::board::{CardTable, Pile, PilePlace, Spot, Spread, Sprite};
use crate::cards::{Card, deck};

pub const DRAW_PILE: u32 = 0;
pub const DISCARD_PILE: u32 = 1;

#[derive(Clone)]
pub struct Table {
    players: Vec<Uuid>,
    hands: Vec<Vec<Card>>,
    draw_pile: Vec<Card>,
    discard_pile: Vec<Card>,
    turn: usize,
    passes: usize,
    shuffle: ChaCha8Rng,
}

impl Table {
    pub fn deal(players: &[Uuid], hand_size: usize, decks: usize) -> Self {
        let mut shuffle = shuffle_for(players);
        let mut draw_pile: Vec<Card> = (0..decks).flat_map(|_| deck()).collect();
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

    pub fn hand(&self) -> &[Card] {
        &self.hands[self.turn]
    }

    pub fn face_up(&self) -> Card {
        *self
            .discard_pile
            .last()
            .expect("the deal turns a card face up")
    }

    pub fn play(&mut self, card: Card) {
        let position = self.hands[self.turn]
            .iter()
            .position(|held| *held == card)
            .expect("a card is only played from the hand holding it");
        self.hands[self.turn].remove(position);
        self.discard_pile.push(card);
        self.passes = 0;
    }

    pub fn can_draw(&self) -> bool {
        !self.draw_pile.is_empty() || self.discard_pile.len() > 1
    }

    pub fn draw(&mut self) -> Option<Card> {
        if self.draw_pile.is_empty() {
            let face_up = self.face_up();
            self.discard_pile.pop();
            self.draw_pile.append(&mut self.discard_pile);
            self.discard_pile.push(face_up);
            self.draw_pile.shuffle(&mut self.shuffle);
        }
        let card = self.draw_pile.pop()?;
        self.hands[self.turn].push(card);
        Some(card)
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
            .rposition(|held| *held == card)
            .expect("only a card in the hand has a place in it");
        Spot::card(self.turn as u32 + 2, position as u32)
    }

    pub fn board(&self, viewer: Uuid) -> CardTable {
        let mut piles = vec![
            Pile {
                label: "Draw pile".to_owned(),
                place: PilePlace::Deck,
                spread: Spread::Stacked,
                cards: vec![Sprite::CardBack; self.draw_pile.len()],
            },
            Pile {
                label: "Discard pile".to_owned(),
                place: PilePlace::Discard,
                spread: Spread::Stacked,
                cards: self
                    .discard_pile
                    .iter()
                    .copied()
                    .map(Sprite::Card)
                    .collect(),
            },
        ];
        for (seat, (player, hand)) in self.players.iter().zip(&self.hands).enumerate() {
            piles.push(if *player == viewer {
                Pile {
                    label: "Your hand".to_owned(),
                    place: PilePlace::Hand,
                    spread: Spread::Fanned,
                    cards: hand.iter().copied().map(Sprite::Card).collect(),
                }
            } else {
                Pile {
                    label: format!("P{}", seat + 1),
                    place: PilePlace::Opponent,
                    spread: Spread::Fanned,
                    cards: vec![Sprite::CardBack; hand.len()],
                }
            });
        }
        CardTable { piles }
    }
}

fn shuffle_for(players: &[Uuid]) -> ChaCha8Rng {
    let seed = players.iter().fold(0u128, |seed, id| seed ^ id.as_u128()) as u64;
    ChaCha8Rng::seed_from_u64(seed)
}

#[cfg(test)]
mod tests;
