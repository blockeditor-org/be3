use std::convert::Infallible;

use uuid::Uuid;

use game_api::board::{Grid, Sprite};
use game_api::{GameHelper, GameScreen, Move, Scene, Spot};

const SIDE: u32 = 3;
const CELL_COUNT: usize = 9;

const LINES: [[usize; 3]; 8] = [
    [0, 1, 2],
    [3, 4, 5],
    [6, 7, 8],
    [0, 3, 6],
    [1, 4, 7],
    [2, 5, 8],
    [0, 4, 8],
    [2, 4, 6],
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Symbol {
    X,
    O,
}

impl Symbol {
    fn label(self) -> &'static str {
        match self {
            Symbol::X => "X",
            Symbol::O => "O",
        }
    }

    fn sprite(self) -> Sprite {
        match self {
            Symbol::X => Sprite::piece("x", 0),
            Symbol::O => Sprite::piece("o", 1),
        }
    }
}

fn spot(cell: usize) -> Spot {
    Spot::tile(cell as u32 % SIDE, cell as u32 / SIDE)
}

fn grid(board: &[Option<Symbol>; CELL_COUNT]) -> Grid {
    let mut grid = Grid::new(SIDE, SIDE);
    for (cell, symbol) in board.iter().enumerate() {
        if let Some(symbol) = symbol {
            grid.place(cell as u32 % SIDE, cell as u32 / SIDE, symbol.sprite());
        }
    }
    grid
}

fn cell_label(cell: usize) -> String {
    format!("{}{}", (b'a' + (cell % 3) as u8) as char, 3 - cell / 3)
}

fn winning_symbol(board: &[Option<Symbol>; CELL_COUNT]) -> Option<Symbol> {
    for line in LINES {
        let [a, b, c] = line.map(|index| board[index]);
        if a.is_some() && a == b && b == c {
            return a;
        }
    }
    None
}

fn tic_tac_toe(helper: GameHelper<'_>) -> Result<Infallible, GameScreen> {
    let mut board: [Option<Symbol>; CELL_COUNT] = [None; CELL_COUNT];
    let mut players: [Option<Uuid>; 2] = [None, None];
    let mut move_count = 0;

    helper.columns(["X", "O"]);
    loop {
        if let Some(winner) = winning_symbol(&board) {
            let score = match winner {
                Symbol::X => "1-0",
                Symbol::O => "0-1",
            };
            return helper.game_over(|_| {
                Scene::new(format!("{} wins!", winner.label()))
                    .score(score)
                    .on(grid(&board))
            });
        }
        if move_count >= CELL_COUNT {
            return helper.game_over(|_| Scene::new("Draw!").score("½-½").on(grid(&board)));
        }

        let turn = move_count % 2;
        let symbol = if turn == 0 { Symbol::X } else { Symbol::O };
        let expected = players[turn];
        let other = players[1 - turn];
        let can_move = move |player: Uuid| match expected {
            Some(expected) => expected == player,
            None => other != Some(player),
        };

        let shown = grid(&board);
        helper.action(
            move |player| {
                let description = if can_move(player) {
                    format!("Your turn ({})", symbol.label())
                } else {
                    format!("Waiting for {}...", symbol.label())
                };
                Scene::new(description).on(shown.clone())
            },
            |player, action| {
                if !can_move(player) {
                    return;
                }
                for (cell, value) in board.iter_mut().enumerate() {
                    if value.is_none()
                        && action(
                            Move::new(cell_label(cell))
                                .click(spot(cell))
                                .column(turn as u32),
                        )
                    {
                        if players[turn].is_none() {
                            players[turn] = Some(player);
                        }
                        *value = Some(symbol);
                        return;
                    }
                }
            },
        )?;

        move_count += 1;
    }
}

game_api::game!("Tic-Tac-Toe", tic_tac_toe);

#[cfg(test)]
mod tests;
