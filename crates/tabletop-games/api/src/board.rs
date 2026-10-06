use serde::{Deserialize, Serialize};

use crate::cards::Card;

pub const CARD_WIDTH: i32 = 70;
pub const CARD_HEIGHT: i32 = 100;
const LABEL_HEIGHT: i32 = 22;
const FAN_STEP: i32 = 36;
const FAN_WIDTH: i32 = 220;
const PILE_GAP: i32 = 28;
const ROW_GAP: i32 = 20;
const LAYOUT: u32 = 0;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct Board {
    pub width: i32,
    pub height: i32,
    pub items: Vec<Item>,
    pub hand: Vec<HandCard>,
}

impl Board {
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            items: Vec::new(),
            hand: Vec::new(),
        }
    }

    pub fn place(&mut self, id: ItemId, area: Area, sprite: Sprite) -> &mut Item {
        self.items.push(Item {
            id,
            area,
            sprite,
            spot: None,
        });
        self.items.last_mut().expect("an item was just placed")
    }

    pub fn hold(&mut self, id: ItemId, sprite: Sprite, spot: Spot) {
        self.hand.push(HandCard { id, sprite, spot });
    }

    pub fn sprites_at(&self, spot: Spot) -> Vec<Sprite> {
        self.items
            .iter()
            .filter(|item| item.spot == Some(spot))
            .map(|item| item.sprite.clone())
            .chain(
                self.hand
                    .iter()
                    .filter(|held| held.spot == spot)
                    .map(|held| held.sprite.clone()),
            )
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Area {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> i32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct ItemId(pub u64);

impl ItemId {
    pub const fn new(group: u32, index: u32) -> Self {
        Self(((group as u64) << 32) | index as u64)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct Item {
    pub id: ItemId,
    pub area: Area,
    pub sprite: Sprite,
    pub spot: Option<Spot>,
}

impl Item {
    pub fn at(&mut self, spot: Spot) -> &mut Self {
        self.spot = Some(spot);
        self
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct HandCard {
    pub id: ItemId,
    pub sprite: Sprite,
    pub spot: Spot,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Sprite {
    Frame,
    Tray,
    Cell,
    Slot,
    Label(String),
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

pub struct Squares {
    columns: u32,
    rows: u32,
    gap: i32,
    inset: i32,
    plate: Sprite,
}

impl Squares {
    pub const TILE: i32 = 64;

    pub fn checkered(columns: u32, rows: u32) -> Self {
        Self {
            columns,
            rows,
            gap: 0,
            inset: 6,
            plate: Sprite::Frame,
        }
    }

    pub fn cells(columns: u32, rows: u32) -> Self {
        Self {
            columns,
            rows,
            gap: 6,
            inset: 6,
            plate: Sprite::Tray,
        }
    }

    pub fn area(&self, column: u32, row: u32) -> Area {
        let step = Self::TILE + self.gap;
        Area::new(
            self.inset + column as i32 * step,
            self.inset + row as i32 * step,
            Self::TILE,
            Self::TILE,
        )
    }

    pub fn board(&self, tile: impl Fn(u32, u32) -> Sprite) -> Board {
        let span = |count: u32| {
            count as i32 * Self::TILE + (count as i32 - 1).max(0) * self.gap + self.inset * 2
        };
        let mut board = Board::new(span(self.columns), span(self.rows));
        let plate = Area::new(0, 0, board.width, board.height);
        board.place(ItemId::new(LAYOUT, 0), plate, self.plate.clone());
        for row in 0..self.rows {
            for column in 0..self.columns {
                let id = ItemId::new(LAYOUT, 1 + row * self.columns + column);
                self.place(&mut board, id, column, row, tile(column, row));
            }
        }
        board
    }

    pub fn place<'b>(
        &self,
        board: &'b mut Board,
        id: ItemId,
        column: u32,
        row: u32,
        sprite: Sprite,
    ) -> &'b mut Item {
        board
            .place(id, self.area(column, row), sprite)
            .at(Spot::tile(column, row))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pile {
    pub label: String,
    pub index: u32,
    pub fanned: bool,
    pub cards: Vec<(ItemId, Sprite)>,
}

impl Pile {
    pub fn stacked(index: u32, label: impl Into<String>, cards: Vec<(ItemId, Sprite)>) -> Self {
        Self {
            label: label.into(),
            index,
            fanned: false,
            cards,
        }
    }

    pub fn fanned(index: u32, label: impl Into<String>, cards: Vec<(ItemId, Sprite)>) -> Self {
        Self {
            fanned: true,
            ..Self::stacked(index, label, cards)
        }
    }

    fn step(&self) -> i32 {
        let gaps = self.cards.len().saturating_sub(1) as i32;
        match gaps {
            0 => FAN_STEP,
            gaps => ((FAN_WIDTH - CARD_WIDTH) / gaps).min(FAN_STEP),
        }
    }

    fn width(&self) -> i32 {
        match self.fanned {
            true => CARD_WIDTH + self.step() * self.cards.len().saturating_sub(1) as i32,
            false => CARD_WIDTH,
        }
    }

    fn place(&self, board: &mut Board, x: i32, y: i32) {
        let label = format!("{} ({})", self.label, self.cards.len());
        let width = self.width();
        board.place(
            ItemId::new(LAYOUT, self.index * 2),
            Area::new(x, y, width.max(CARD_WIDTH * 2), LABEL_HEIGHT),
            Sprite::Label(label),
        );
        let top = y + LABEL_HEIGHT;
        board
            .place(
                ItemId::new(LAYOUT, self.index * 2 + 1),
                Area::new(x, top, width, CARD_HEIGHT),
                Sprite::Slot,
            )
            .at(Spot::Pile(self.index));
        let step = self.step();
        for (card, (id, sprite)) in self.cards.iter().enumerate() {
            let (left, spot) = match self.fanned {
                true => (x + step * card as i32, Spot::card(self.index, card as u32)),
                false => (x, Spot::Pile(self.index)),
            };
            board
                .place(
                    *id,
                    Area::new(left, top, CARD_WIDTH, CARD_HEIGHT),
                    sprite.clone(),
                )
                .at(spot);
        }
    }
}

pub fn card_table(rows: Vec<Vec<Pile>>) -> Board {
    let row_width = |row: &[Pile]| {
        row.iter().map(Pile::width).sum::<i32>() + PILE_GAP * (row.len() as i32 - 1).max(0)
    };
    let width = rows
        .iter()
        .map(|row| row_width(row))
        .max()
        .unwrap_or(0)
        .max(FAN_WIDTH * 2 + PILE_GAP);
    let row_height = LABEL_HEIGHT + CARD_HEIGHT;
    let height = rows.len() as i32 * row_height + (rows.len() as i32 - 1).max(0) * ROW_GAP;
    let mut board = Board::new(width, height);
    for (number, row) in rows.iter().enumerate() {
        let mut x = (width - row_width(row)) / 2;
        let y = number as i32 * (row_height + ROW_GAP);
        for pile in row {
            pile.place(&mut board, x, y);
            x += pile.width() + PILE_GAP;
        }
    }
    board
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
    Control(Control),
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Control {
    Resign,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    pub label: String,
    pub gesture: Option<Gesture>,
    pub recorded: Option<String>,
    pub column: Option<u32>,
}

impl Move {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            gesture: None,
            recorded: None,
            column: None,
        }
    }

    pub fn column(mut self, column: u32) -> Self {
        self.column = Some(column);
        self
    }

    pub fn control(mut self, control: Control) -> Self {
        self.gesture = Some(Gesture::Control(control));
        self
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
    pub score: Option<String>,
}

impl Scene {
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            board: Board::default(),
            score: None,
        }
    }

    pub fn score(mut self, score: impl Into<String>) -> Self {
        self.score = Some(score.into());
        self
    }

    pub fn on(mut self, board: Board) -> Self {
        self.board = board;
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

#[cfg(test)]
mod tests;
