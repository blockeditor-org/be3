use std::convert::Infallible;

use game_api::{GameHelper, GameScreen};
use game_pieces::{Army, Notation, Position, Rules, Side, play};

const RANKS_OF_MEN: i8 = 3;

pub static CHECKERS: Rules = Rules {
    columns: 8,
    rows: 8,
    setup,
    armies: [
        Army {
            name: "Dark",
            color: 1,
            must_capture: true,
        },
        Army {
            name: "Light",
            color: 0,
            must_capture: true,
        },
    ],
    notation: Notation::Numbered,
    quiet_limit: 80,
};

fn setup(position: &mut Position) {
    game_pieces::checkers::army(position, Side::First, RANKS_OF_MEN);
    game_pieces::checkers::army(position, Side::Second, RANKS_OF_MEN);
}

fn checkers(helper: GameHelper<'_>) -> Result<Infallible, GameScreen> {
    play(helper, &CHECKERS)
}

game_api::game!("Checkers", checkers);

#[cfg(test)]
mod tests;
