use std::convert::Infallible;

use game_api::{GameHelper, GameScreen};
use game_pieces::{Army, Notation, Position, Rules, Side, play};

const RANKS_OF_MEN: i8 = 3;

pub static CHESS_VS_CHECKERS: Rules = Rules {
    columns: 8,
    rows: 8,
    setup,
    armies: [
        Army {
            name: "Chess",
            color: 0,
            must_capture: false,
        },
        Army {
            name: "Checkers",
            color: 1,
            must_capture: true,
        },
    ],
    notation: Notation::Algebraic,
    quiet_limit: 100,
};

fn setup(position: &mut Position) {
    game_pieces::chess::army(position, Side::First);
    game_pieces::checkers::army(position, Side::Second, RANKS_OF_MEN);
}

fn chess_vs_checkers(helper: GameHelper<'_>) -> Result<Infallible, GameScreen> {
    play(helper, &CHESS_VS_CHECKERS)
}

game_api::game!("Chess vs Checkers", chess_vs_checkers);

#[cfg(test)]
mod tests;
