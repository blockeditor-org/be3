use std::convert::Infallible;

use game_api::board::{Board, CardTable, Pile, PilePlace, Spread, Sprite};
use game_api::cards::deck;
use game_api::cards::{Card, Rank, SUITS, Suit};
use game_api::table::{DISCARD_PILE, DRAW_PILE, Table};
use game_api::{Choose, GameHelper, GameScreen, Move, Scene, Spot};
use uuid::Uuid;

const HAND_SIZE: usize = 8;
const PLAYERS_PER_DECK: usize = 4;
const WAITING: &str = "Waiting for your turn...";

fn crazy_8s(helper: GameHelper<'_>) -> Result<Infallible, GameScreen> {
    let players = helper.gather(2, Spot::Pile(DRAW_PILE), seating)?;
    helper.columns((1..=players.len()).map(|seat| format!("P{seat}")));
    let mut table = Table::deal(&players, HAND_SIZE, decks_for(players.len()));
    let mut suit_to_match = table.face_up().suit;

    loop {
        if let Some(winner) = table.player_who_is_out() {
            let seat = seat_of(&players, winner);
            return helper.game_over(|player| {
                let outcome = if player == winner {
                    "You win!".to_owned()
                } else if players.contains(&player) {
                    format!("You lose - P{seat} wins")
                } else {
                    format!("P{seat} wins")
                };
                Scene::new(outcome).on(table.board(player))
            });
        }
        if table.everyone_has_passed() {
            return helper
                .game_over(|player| Scene::new("Draw! No one can play.").on(table.board(player)));
        }

        let whose_turn = table.whose_turn();
        let column = seat_of(&players, whose_turn) as u32 - 1;
        let your_turn = format!(
            "Your turn - top card is {} ({suit_to_match} to match)",
            table.face_up()
        );
        let mut drawn = None;

        let seen = table.clone();
        helper.turn(
            whose_turn,
            &your_turn,
            WAITING,
            |player| seen.board(player).into(),
            |choose| {
                let choose = &mut |offered: Move| choose(offered.column(column));
                let hand = table.hand().to_vec();
                if play_a_card(&mut table, &mut suit_to_match, &hand, choose) {
                    return;
                }
                if table.can_draw() {
                    let draw = Move::new("Draw a card")
                        .click(Spot::Pile(DRAW_PILE))
                        .recorded("draw");
                    if choose(draw) {
                        drawn = table.draw();
                    }
                } else if choose(
                    Move::new("Pass")
                        .click(Spot::Pile(DRAW_PILE))
                        .recorded("pass"),
                ) {
                    table.pass();
                }
            },
        )?;

        if let Some(card) =
            drawn.filter(|card| can_be_played(*card, table.face_up(), suit_to_match))
        {
            let you_drew = format!("You drew the {card} - play it or keep it");
            let seen = table.clone();
            helper.turn(
                whose_turn,
                &you_drew,
                WAITING,
                |player| seen.board(player).into(),
                |choose| {
                    let choose = &mut |offered: Move| choose(offered.column(column));
                    if play_a_card(&mut table, &mut suit_to_match, &[card], choose) {
                        return;
                    }
                    choose(
                        Move::new("Keep it")
                            .click(Spot::Pile(DRAW_PILE))
                            .recorded("keep"),
                    );
                },
            )?;
        }

        table.turn_passes_to_the_left();
    }
}

fn play_a_card(
    table: &mut Table,
    suit_to_match: &mut Suit,
    cards: &[Card],
    choose: &mut Choose<'_>,
) -> bool {
    for card in cards.iter().copied() {
        if !can_be_played(card, table.face_up(), *suit_to_match) {
            continue;
        }
        let from = table.in_hand(card);
        let onto_the_discard_pile = move |label: String, history: String| {
            Move::new(label)
                .drag(from, Spot::Pile(DISCARD_PILE))
                .recorded(history)
        };
        if card.rank == Rank::Eight {
            for suit in SUITS {
                if choose(onto_the_discard_pile(
                    format!("Play {card} and call {suit}"),
                    format!("{}→{}", card.short(), suit.symbol()),
                )) {
                    table.play(card);
                    *suit_to_match = suit;
                    return true;
                }
            }
        } else if choose(onto_the_discard_pile(format!("Play {card}"), card.short())) {
            table.play(card);
            *suit_to_match = card.suit;
            return true;
        }
    }
    false
}

fn seat_of(players: &[Uuid], player: Uuid) -> usize {
    players
        .iter()
        .position(|seated| *seated == player)
        .map_or(0, |seat| seat + 1)
}

fn seating(joined: &[Uuid], viewer: Uuid) -> Board {
    let mut piles = vec![Pile {
        label: "Deck".to_owned(),
        place: PilePlace::Deck,
        spread: Spread::Stacked,
        cards: vec![Sprite::CardBack; deck().len()],
    }];
    for (seat, player) in joined.iter().enumerate() {
        piles.push(Pile {
            label: match *player == viewer {
                true => "You".to_owned(),
                false => format!("P{}", seat + 1),
            },
            place: match *player == viewer {
                true => PilePlace::Hand,
                false => PilePlace::Opponent,
            },
            spread: Spread::Fanned,
            cards: Vec::new(),
        });
    }
    CardTable { piles }.into()
}

fn can_be_played(card: Card, face_up: Card, suit_to_match: Suit) -> bool {
    card.rank == Rank::Eight || card.suit == suit_to_match || card.rank == face_up.rank
}

fn decks_for(players: usize) -> usize {
    players.div_ceil(PLAYERS_PER_DECK)
}

game_api::game!("Crazy 8s", crazy_8s);

#[cfg(test)]
mod tests;
