use serde::{Deserialize, Serialize};

use crate::cards::Card;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Board {
    #[default]
    Empty,
    Grid(Grid),
    Cards(CardTable),
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct Grid {
    pub columns: u32,
    pub rows: u32,
    pub tiles: Vec<Tile>,
}

impl Grid {
    pub fn new(columns: u32, rows: u32) -> Self {
        Self {
            columns,
            rows,
            tiles: vec![Tile::default(); (columns * rows) as usize],
        }
    }

    pub fn tile(&self, column: u32, row: u32) -> &Tile {
        &self.tiles[(row * self.columns + column) as usize]
    }

    pub fn tile_mut(&mut self, column: u32, row: u32) -> &mut Tile {
        &mut self.tiles[(row * self.columns + column) as usize]
    }

    pub fn place(&mut self, column: u32, row: u32, sprite: Sprite) {
        self.tile_mut(column, row).layers.push(sprite);
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct Tile {
    pub layers: Vec<Sprite>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Sprite {
    Square(Shade),
    Tint(Tint),
    Piece(Piece),
    Card(Card),
    CardBack,
}

impl Sprite {
    pub fn piece(kind: impl Into<String>, seat: u8) -> Self {
        Self::Piece(Piece {
            kind: kind.into(),
            seat,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Shade {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Tint {
    LastMove,
    Danger,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct Piece {
    pub kind: String,
    pub seat: u8,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct CardTable {
    pub piles: Vec<Pile>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct Pile {
    pub label: String,
    pub place: PilePlace,
    pub spread: Spread,
    pub cards: Vec<Sprite>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum PilePlace {
    Deck,
    Discard,
    Extra,
    Hand,
    Opponent,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Spread {
    Stacked,
    Fanned,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Spot {
    Tile { column: u32, row: u32 },
    Pile(u32),
    Card { pile: u32, card: u32 },
}

impl Spot {
    pub fn tile(column: u32, row: u32) -> Self {
        Self::Tile { column, row }
    }

    pub fn card(pile: u32, card: u32) -> Self {
        Self::Card { pile, card }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Gesture {
    Click(Spot),
    Drag { from: Spot, to: Spot },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    pub label: String,
    pub gesture: Option<Gesture>,
    pub recorded: Option<String>,
}

impl Move {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            gesture: None,
            recorded: None,
        }
    }

    pub fn recorded(mut self, history: impl Into<String>) -> Self {
        self.recorded = Some(history.into());
        self
    }

    pub fn click(mut self, spot: Spot) -> Self {
        self.gesture = Some(Gesture::Click(spot));
        self
    }

    pub fn drag(mut self, from: Spot, to: Spot) -> Self {
        self.gesture = Some(Gesture::Drag { from, to });
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scene {
    pub description: String,
    pub board: Board,
}

impl Scene {
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            board: Board::Empty,
        }
    }

    pub fn on(mut self, board: impl Into<Board>) -> Self {
        self.board = board.into();
        self
    }
}

impl From<String> for Scene {
    fn from(description: String) -> Self {
        Self::new(description)
    }
}

impl From<&str> for Scene {
    fn from(description: &str) -> Self {
        Self::new(description)
    }
}

impl From<Grid> for Board {
    fn from(grid: Grid) -> Self {
        Self::Grid(grid)
    }
}

impl From<CardTable> for Board {
    fn from(table: CardTable) -> Self {
        Self::Cards(table)
    }
}
