use game_pieces::Side;

use crate::CHESS;

use super::*;

const PLIES: usize = 400;

#[test]
fn a_long_game_gives_every_move_fuel_of_its_own() {
    let mut position = CHESS.start();
    let mut side = Side::First;
    let mut actions = Vec::new();
    let mut seed = 7u64;
    for _ in 0..PLIES {
        let legal = position.legal(side, false);
        let quiet: Vec<usize> = (0..legal.len())
            .filter(|index| !legal[*index].is_capture())
            .collect();
        let Some(choice) = (!quiet.is_empty()).then(|| {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            quiet[(seed >> 33) as usize % quiet.len()]
        }) else {
            break;
        };
        position.apply(&legal[choice]);
        actions.push(GameAction {
            actor: if side == Side::First { FIRST } else { SECOND },
            action: bincode::serialize(&(choice as u32)).expect("an index encodes"),
        });
        side = side.other();
    }

    let screen = show(&actions, FIRST);

    assert!(screen.history.len() >= 150, "{}", screen.history.len());
}
