use block_editor_beui::beui::{Pos2, Rect, Vec2, pos2, vec2};
use game_api::Spot;
use game_api::board::{Board, CardTable, Grid, Pile, PilePlace, Spread, Sprite};

pub(crate) const TILE: f32 = 64.0;
pub(crate) const CARD: Vec2 = Vec2::new(70.0, 100.0);
const CELL_GAP: f32 = 6.0;
const MARGIN: f32 = 12.0;
const BORDER: f32 = 6.0;
const FAN_STEP: f32 = 36.0;
const HAND_WIDTH: f32 = 560.0;
const OPPONENT_WIDTH: f32 = 220.0;
const PILE_GAP: f32 = 28.0;
const ROW_GAP: f32 = 20.0;
pub(crate) const LABEL_HEIGHT: f32 = 22.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Look {
    Square,
    Cell,
    Card,
    Pile,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Placed {
    pub(crate) spot: Spot,
    pub(crate) rect: Rect,
    pub(crate) layers: Vec<Sprite>,
    pub(crate) look: Look,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Label {
    pub(crate) text: String,
    pub(crate) rect: Rect,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Layout {
    pub(crate) size: Vec2,
    pub(crate) plate: Option<Rect>,
    pub(crate) spots: Vec<Placed>,
    pub(crate) labels: Vec<Label>,
}

impl Layout {
    pub(crate) fn of(board: &Board) -> Self {
        match board {
            Board::Empty => Self::default(),
            Board::Grid(grid) => grid_layout(grid),
            Board::Cards(table) => table_layout(table),
        }
    }

    pub(crate) fn hit(&self, point: Pos2) -> Option<Spot> {
        self.spots
            .iter()
            .rev()
            .find(|placed| placed.rect.contains(point))
            .map(|placed| placed.spot)
    }

    pub(crate) fn find(&self, spot: Spot) -> Option<&Placed> {
        self.spots.iter().find(|placed| placed.spot == spot)
    }

    pub(crate) fn centered_in(&self, world: Option<Vec2>) -> Self {
        let Some(world) = world else {
            return self.clone();
        };
        let by = ((world - self.size) / 2.0).max(Vec2::ZERO);
        if by == Vec2::ZERO {
            return self.clone();
        }
        Self {
            size: self.size + by,
            plate: self.plate.map(|plate| plate.translate(by)),
            spots: self
                .spots
                .iter()
                .map(|placed| Placed {
                    rect: placed.rect.translate(by),
                    ..placed.clone()
                })
                .collect(),
            labels: self
                .labels
                .iter()
                .map(|label| Label {
                    rect: label.rect.translate(by),
                    ..label.clone()
                })
                .collect(),
        }
    }

    pub(crate) fn bounds(&self) -> Rect {
        Rect::from_min_size(Pos2::ZERO, self.size)
    }
}

fn grid_layout(grid: &Grid) -> Layout {
    let checkered = !grid.tiles.is_empty()
        && grid
            .tiles
            .iter()
            .all(|tile| matches!(tile.layers.first(), Some(Sprite::Square(_))));
    let (gap, inset) = match checkered {
        true => (0.0, BORDER),
        false => (CELL_GAP, CELL_GAP),
    };
    let origin = pos2(MARGIN + inset, MARGIN + inset);
    let mut spots = Vec::with_capacity(grid.tiles.len());
    for row in 0..grid.rows {
        for column in 0..grid.columns {
            let min = origin + vec2(column as f32 * (TILE + gap), row as f32 * (TILE + gap));
            spots.push(Placed {
                spot: Spot::tile(column, row),
                rect: Rect::from_min_size(min, Vec2::splat(TILE)),
                layers: grid.tile(column, row).layers.clone(),
                look: match checkered {
                    true => Look::Square,
                    false => Look::Cell,
                },
            });
        }
    }
    let span = |count: u32| count as f32 * TILE + count.saturating_sub(1) as f32 * gap;
    let plate = Rect::from_min_size(
        pos2(MARGIN, MARGIN),
        vec2(span(grid.columns), span(grid.rows)) + Vec2::splat(inset * 2.0),
    );
    Layout {
        size: plate.max.to_vec2() + Vec2::splat(MARGIN),
        plate: Some(plate),
        spots,
        labels: Vec::new(),
    }
}

struct Row {
    piles: Vec<(u32, f32, f32)>,
    width: f32,
}

fn fan_step(count: usize, widest: f32) -> f32 {
    if count < 2 {
        return FAN_STEP;
    }
    ((widest - CARD.x) / (count - 1) as f32).min(FAN_STEP)
}

fn table_layout(table: &CardTable) -> Layout {
    let rows = [
        &[PilePlace::Opponent][..],
        &[PilePlace::Deck, PilePlace::Discard, PilePlace::Extra][..],
        &[PilePlace::Hand][..],
    ]
    .map(|places| {
        let piles: Vec<(u32, f32, f32)> = table
            .piles
            .iter()
            .enumerate()
            .filter(|(_, pile)| places.contains(&pile.place))
            .map(|(index, pile)| {
                let step = fan_step(pile.cards.len(), widest(pile));
                let width = match pile.spread {
                    Spread::Stacked => CARD.x,
                    Spread::Fanned => CARD.x + step * pile.cards.len().saturating_sub(1) as f32,
                };
                (index as u32, width, step)
            })
            .collect();
        let width = piles.iter().map(|(_, width, _)| width).sum::<f32>()
            + PILE_GAP * piles.len().saturating_sub(1) as f32;
        Row { piles, width }
    });
    let widest_row = rows.iter().map(|row| row.width).fold(0.0, f32::max);
    let mut layout = Layout::default();
    let mut top = MARGIN;
    for row in rows.iter().filter(|row| !row.piles.is_empty()) {
        let mut left = MARGIN + (widest_row - row.width) / 2.0;
        for (index, width, step) in &row.piles {
            let pile = &table.piles[*index as usize];
            place_pile(&mut layout, pile, *index, pos2(left, top), *width, *step);
            left += width + PILE_GAP;
        }
        top += LABEL_HEIGHT + CARD.y + ROW_GAP;
    }
    layout.size = vec2(widest_row + MARGIN * 2.0, top - ROW_GAP + MARGIN);
    layout
}

fn widest(pile: &Pile) -> f32 {
    match pile.place {
        PilePlace::Opponent => OPPONENT_WIDTH,
        _ => HAND_WIDTH,
    }
}

fn place_pile(layout: &mut Layout, pile: &Pile, index: u32, at: Pos2, width: f32, step: f32) {
    layout.labels.push(Label {
        text: format!("{} ({})", pile.label, pile.cards.len()),
        rect: Rect::from_min_size(at, vec2(width.max(CARD.x * 2.0), LABEL_HEIGHT)),
    });
    let top = at + vec2(0.0, LABEL_HEIGHT);
    match pile.spread {
        Spread::Stacked => layout.spots.push(Placed {
            spot: Spot::Pile(index),
            rect: Rect::from_min_size(top, CARD),
            layers: pile.cards.last().cloned().into_iter().collect(),
            look: Look::Pile,
        }),
        Spread::Fanned => {
            layout.spots.push(Placed {
                spot: Spot::Pile(index),
                rect: Rect::from_min_size(top, vec2(width, CARD.y)),
                layers: Vec::new(),
                look: Look::Pile,
            });
            for (card, sprite) in pile.cards.iter().enumerate() {
                layout.spots.push(Placed {
                    spot: Spot::card(index, card as u32),
                    rect: Rect::from_min_size(top + vec2(step * card as f32, 0.0), CARD),
                    layers: vec![sprite.clone()],
                    look: Look::Card,
                });
            }
        }
    }
}
