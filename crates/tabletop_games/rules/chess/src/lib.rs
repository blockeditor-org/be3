use std::convert::Infallible;

use game_api::{GameHelper, GameScreen};
use game_pieces::{Army, Notation, Position, Rules, Side, play};

pub static CHESS: Rules = Rules {
    columns: 8,
    rows: 8,
    setup,
    armies: [
        Army {
            name: "White",
            color: 0,
            must_capture: false,
        },
        Army {
            name: "Black",
            color: 1,
            must_capture: false,
        },
    ],
    notation: Notation::Algebraic,
    quiet_limit: 100,
};

fn setup(position: &mut Position) {
    game_pieces::chess::army(position, Side::First);
    game_pieces::chess::army(position, Side::Second);
}

fn chess(helper: GameHelper<'_>) -> Result<Infallible, GameScreen> {
    play(helper, &CHESS)
}

game_api::game!("Chess", chess);

#[cfg(test)]
mod tests;
