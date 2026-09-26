use std::sync::OnceLock;

use game_host::{Game, GameAction, GameScreen};
use uuid::Uuid;

mod a_man_jumps_a_chess_piece_when_it_can;
mod chess_opens_and_checkers_answers_with_its_men;

const FIRST: Uuid = Uuid::from_u128(1);
const SECOND: Uuid = Uuid::from_u128(2);

fn show(actions: &[GameAction], player: Uuid) -> GameScreen {
    static GAME: OnceLock<Game> = OnceLock::new();
    GAME.get_or_init(|| {
        Game::load(include_bytes!(env!("GAME_WASM"))).expect("this crate builds its own module")
    })
    .show(actions, player)
    .expect("the module answers every screen it is asked for")
}

fn labels(actions: &[GameAction], player: Uuid) -> Vec<String> {
    show(actions, player)
        .actions
        .into_iter()
        .map(|option| option.label)
        .collect()
}

fn played(moves: &[(Uuid, &str)]) -> Vec<GameAction> {
    let mut actions = Vec::new();
    for (actor, label) in moves {
        let option = show(&actions, *actor)
            .actions
            .into_iter()
            .find(|option| option.label == *label)
            .unwrap_or_else(|| panic!("{label} is not a legal move for {actor}"));
        actions.push(GameAction {
            actor: *actor,
            action: option.effect,
        });
    }
    actions
}
