use std::cell::Cell;
use std::rc::Rc;

use block::Block;
use block_client::BlockClient;
use reactive::{Scope, create_effect};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::BlockSource;

mod an_idle_pump_does_not_run_its_projections_again;
mod editing_one_item_wakes_only_the_bindings_that_read_it;
mod operating_through_the_source_is_visible_before_it_returns;

fn client() -> BlockClient {
    BlockClient::new(Uuid::new_v4(), Uuid::new_v4())
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct Deck {
    cards: Vec<Card>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct Card {
    id: Uuid,
    face: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
enum DeckOperation {
    Insert { card: Card, index: usize },
    Turn { id: Uuid, face: Uuid },
}

impl Deck {
    fn cards(&self) -> &[Card] {
        &self.cards
    }
}

impl Block for Deck {
    type Operation = DeckOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x6465_636b);

    fn apply_operation(deck: &mut Self, operation: &Self::Operation) {
        match operation {
            DeckOperation::Insert { card, index } => {
                deck.cards
                    .insert((*index).min(deck.cards.len()), card.clone());
            }
            DeckOperation::Turn { id, face } => {
                if let Some(card) = deck.cards.iter_mut().find(|card| card.id == *id) {
                    card.face = *face;
                }
            }
        }
    }
}

fn add_card(index: usize) -> DeckOperation {
    DeckOperation::Insert {
        card: Card {
            id: Uuid::new_v4(),
            face: Uuid::new_v4(),
        },
        index,
    }
}
