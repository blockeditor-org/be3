use std::convert::Infallible;

use tabletop_api::{GameHelper, GameScreen};
use tabletop_pieces::{Army, Notation, Position, Rules, Side, play};

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
    tabletop_pieces::checkers::army(position, Side::First, RANKS_OF_MEN);
    tabletop_pieces::checkers::army(position, Side::Second, RANKS_OF_MEN);
}

fn checkers(helper: GameHelper<'_>) -> Result<Infallible, GameScreen> {
    play(helper, &CHECKERS)
}

tabletop_api::game!("Checkers", checkers);

#[cfg(test)]
mod tests;
