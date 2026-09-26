use std::convert::Infallible;

use uuid::Uuid;

use game_api::board::{Grid, Sprite};
use game_api::{GameHelper, GameScreen, Move, Scene, Spot};

const COLUMNS: usize = 7;
const ROWS: usize = 6;
const CELL_COUNT: usize = COLUMNS * ROWS;

const DIRECTIONS: [(isize, isize); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Symbol {
    Red,
    Yellow,
}

impl Symbol {
    fn label(self) -> &'static str {
        match self {
            Symbol::Red => "Red",
            Symbol::Yellow => "Yellow",
        }
    }

    fn sprite(self) -> Sprite {
        match self {
            Symbol::Red => Sprite::piece("disc", 0),
            Symbol::Yellow => Sprite::piece("disc", 1),
        }
    }
}

fn spot(column: usize, row: usize) -> Spot {
    Spot::tile(column as u32, (ROWS - 1 - row) as u32)
}

fn grid(board: &[Option<Symbol>; CELL_COUNT]) -> Grid {
    let mut grid = Grid::new(COLUMNS as u32, ROWS as u32);
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            if let Some(symbol) = board[cell(column, row)] {
                grid.place(column as u32, (ROWS - 1 - row) as u32, symbol.sprite());
            }
        }
    }
    grid
}

fn cell(column: usize, row: usize) -> usize {
    row * COLUMNS + column
}

fn column_label(column: usize) -> String {
    (column + 1).to_string()
}

fn in_bounds(column: isize, row: isize) -> bool {
    (0..COLUMNS as isize).contains(&column) && (0..ROWS as isize).contains(&row)
}

fn winning_symbol(board: &[Option<Symbol>; CELL_COUNT]) -> Option<Symbol> {
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            let Some(symbol) = board[cell(column, row)] else {
                continue;
            };
            for (delta_column, delta_row) in DIRECTIONS {
                let in_a_row = (0..4).all(|step| {
                    let column = column as isize + delta_column * step;
                    let row = row as isize + delta_row * step;
                    in_bounds(column, row)
                        && board[cell(column as usize, row as usize)] == Some(symbol)
                });
                if in_a_row {
                    return Some(symbol);
                }
            }
        }
    }
    None
}

fn connect_four(helper: GameHelper<'_>) -> Result<Infallible, GameScreen> {
    let mut board: [Option<Symbol>; CELL_COUNT] = [None; CELL_COUNT];
    let mut column_heights: [usize; COLUMNS] = [0; COLUMNS];
    let mut players: [Option<Uuid>; 2] = [None, None];
    let mut move_count = 0;

    helper.columns(["Red", "Yellow"]);
    loop {
        if let Some(winner) = winning_symbol(&board) {
            let score = match winner {
                Symbol::Red => "1-0",
                Symbol::Yellow => "0-1",
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
        let symbol = if turn == 0 {
            Symbol::Red
        } else {
            Symbol::Yellow
        };
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
                for column in 0..COLUMNS {
                    let height = column_heights[column];
                    if height < ROWS
                        && action(
                            Move::new(column_label(column))
                                .click(spot(column, height))
                                .column(turn as u32),
                        )
                    {
                        if players[turn].is_none() {
                            players[turn] = Some(player);
                        }
                        board[cell(column, height)] = Some(symbol);
                        column_heights[column] += 1;
                        return;
                    }
                }
            },
        )?;

        move_count += 1;
    }
}

game_api::game!("Connect Four", connect_four);

#[cfg(test)]
mod tests;
