use block_editor_beui::beui::{Pos2, Rect, Vec2, pos2, vec2};
use game_api::Spot;
use game_api::board::{Area, Board, ItemId, Sprite};

const EXTENT: f32 = 524.0;
const MARGIN: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Look {
    Square,
    Cell,
    Card,
    Plain,
}

impl Look {
    fn of(sprite: &Sprite) -> Self {
        match sprite {
            Sprite::Square(_) | Sprite::Tint(_) | Sprite::Piece(_) => Look::Square,
            Sprite::Cell => Look::Cell,
            Sprite::Slot | Sprite::Card(_) | Sprite::CardBack => Look::Card,
            Sprite::Frame | Sprite::Tray | Sprite::Label(_) => Look::Plain,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Placed {
    pub(crate) id: ItemId,
    pub(crate) rect: Rect,
    pub(crate) sprite: Sprite,
    pub(crate) spot: Option<Spot>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SpotPlace {
    pub(crate) spot: Spot,
    pub(crate) rect: Rect,
    pub(crate) look: Look,
    pub(crate) occupied: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Layout {
    pub(crate) size: Vec2,
    pub(crate) unit: f32,
    pub(crate) items: Vec<Placed>,
    pub(crate) spots: Vec<SpotPlace>,
}

pub(crate) fn movable(sprite: &Sprite) -> bool {
    matches!(
        sprite,
        Sprite::Piece(_) | Sprite::Card(_) | Sprite::CardBack
    )
}

impl Layout {
    pub(crate) fn of(board: &Board) -> Self {
        if board.width <= 0 || board.height <= 0 {
            return Self::default();
        }
        let unit = EXTENT / board.width.max(board.height) as f32;
        let to_world = |area: &Area| {
            Rect::from_min_size(
                pos2(MARGIN + area.x as f32 * unit, MARGIN + area.y as f32 * unit),
                vec2(area.width as f32 * unit, area.height as f32 * unit),
            )
        };
        let items: Vec<Placed> = board
            .items
            .iter()
            .map(|item| Placed {
                id: item.id,
                rect: to_world(&item.area),
                sprite: item.sprite.clone(),
                spot: item.spot,
            })
            .collect();
        let mut spots: Vec<SpotPlace> = Vec::new();
        for placed in &items {
            let Some(spot) = placed.spot else {
                continue;
            };
            let piece = matches!(placed.sprite, Sprite::Piece(_));
            match spots.iter_mut().find(|known| known.spot == spot) {
                Some(known) => {
                    known.rect = known.rect.union(placed.rect);
                    known.occupied |= piece;
                }
                None => spots.push(SpotPlace {
                    spot,
                    rect: placed.rect,
                    look: Look::of(&placed.sprite),
                    occupied: piece,
                }),
            }
        }
        Self {
            size: vec2(
                board.width as f32 * unit + MARGIN * 2.0,
                board.height as f32 * unit + MARGIN * 2.0,
            ),
            unit,
            items,
            spots,
        }
    }

    pub(crate) fn hit(&self, point: Pos2) -> Option<Spot> {
        self.items
            .iter()
            .rev()
            .find(|placed| placed.spot.is_some() && placed.rect.contains(point))
            .and_then(|placed| placed.spot)
    }

    pub(crate) fn find(&self, spot: Spot) -> Option<&SpotPlace> {
        self.spots.iter().find(|place| place.spot == spot)
    }

    pub(crate) fn item(&self, id: ItemId) -> Option<&Placed> {
        self.items.iter().find(|placed| placed.id == id)
    }

    pub(crate) fn lifted(&self, spot: Spot) -> Option<&Placed> {
        self.items
            .iter()
            .rev()
            .find(|placed| placed.spot == Some(spot) && movable(&placed.sprite))
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
            unit: self.unit,
            items: self
                .items
                .iter()
                .map(|placed| Placed {
                    rect: placed.rect.translate(by),
                    ..placed.clone()
                })
                .collect(),
            spots: self
                .spots
                .iter()
                .map(|place| SpotPlace {
                    rect: place.rect.translate(by),
                    ..place.clone()
                })
                .collect(),
        }
    }

    pub(crate) fn bounds(&self) -> Rect {
        Rect::from_min_size(Pos2::ZERO, self.size)
    }
}
