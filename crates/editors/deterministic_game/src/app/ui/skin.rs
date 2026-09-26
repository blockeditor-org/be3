use std::f32::consts::FRAC_PI_4;

use block_editor_beui::beui::{Color32, FontId, Painter, Pos2, Rect, Vec2, pos2, vec2};
use game_api::board::{Piece, Shade, Sprite, Tint};
use game_api::cards::{Card, Rank, Suit};

const SEATS: [Color32; 4] = [
    Color32::from_rgb(214, 69, 69),
    Color32::from_rgb(230, 180, 34),
    Color32::from_rgb(59, 130, 246),
    Color32::from_rgb(34, 160, 107),
];
const LIGHT_SQUARE: Color32 = Color32::from_rgb(238, 226, 200);
const DARK_SQUARE: Color32 = Color32::from_rgb(150, 111, 81);
const LAST_MOVE: Color32 = Color32::from_rgba_unmultiplied(205, 210, 60, 110);
const DANGER: Color32 = Color32::from_rgba_unmultiplied(220, 40, 40, 150);
const IVORY: Color32 = Color32::from_rgb(248, 245, 236);
const IVORY_EDGE: Color32 = Color32::from_rgb(34, 34, 38);
const EBONY: Color32 = Color32::from_rgb(52, 50, 56);
const EBONY_EDGE: Color32 = Color32::from_rgb(222, 218, 210);
const GOLD: Color32 = Color32::from_rgb(230, 180, 34);
const CARD_FACE: Color32 = Color32::from_rgb(250, 250, 247);
const CARD_EDGE: Color32 = Color32::from_rgb(190, 190, 184);
const CARD_BACK: Color32 = Color32::from_rgb(44, 82, 160);
const CARD_BACK_TRIM: Color32 = Color32::from_rgb(160, 186, 235);
const RED_SUIT: Color32 = Color32::from_rgb(200, 36, 44);
const BLACK_SUIT: Color32 = Color32::from_rgb(28, 28, 32);
const OUTLINE: f32 = 0.035;

pub(crate) fn paint_sprite(painter: &Painter, rect: Rect, sprite: &Sprite) {
    match sprite {
        Sprite::Square(Shade::Light) => painter.rect_filled(rect, 0.0, LIGHT_SQUARE),
        Sprite::Square(Shade::Dark) => painter.rect_filled(rect, 0.0, DARK_SQUARE),
        Sprite::Tint(Tint::LastMove) => painter.rect_filled(rect, 0.0, LAST_MOVE),
        Sprite::Tint(Tint::Danger) => {
            let size = rect.width().min(rect.height());
            painter.rect_filled(rect.shrink(size * 0.06), size * 0.44, DANGER);
        }
        Sprite::Piece(piece) => paint_piece(painter, square_in(rect), piece),
        Sprite::Card(card) => paint_card(painter, rect, *card),
        Sprite::CardBack => paint_back(painter, rect),
    }
}

fn square_in(rect: Rect) -> Rect {
    Rect::from_center_size(rect.center(), Vec2::splat(rect.width().min(rect.height())))
}

#[derive(Clone, Copy)]
enum Shape {
    Box(f32, f32, f32, f32, f32),
    Disc(f32, f32, f32),
    Bar(f32, f32, f32, f32, f32),
    Tilted(f32, f32, f32, f32, f32, f32),
}

struct Unit {
    rect: Rect,
}

impl Unit {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.rect.min + vec2(x * self.rect.width(), y * self.rect.height())
    }

    fn size(&self, length: f32) -> f32 {
        length * self.rect.width()
    }

    fn paint(&self, painter: &Painter, shape: Shape, grow: f32, color: Color32) {
        let grow = self.size(grow);
        match shape {
            Shape::Box(left, top, right, bottom, radius) => painter.rect_filled(
                Rect::from_min_max(self.at(left, top), self.at(right, bottom)).expand(grow),
                self.size(radius) + grow,
                color,
            ),
            Shape::Disc(x, y, radius) => {
                let radius = self.size(radius) + grow;
                painter.rect_filled(
                    Rect::from_center_size(self.at(x, y), Vec2::splat(radius * 2.0)),
                    radius,
                    color,
                );
            }
            Shape::Bar(x0, y0, x1, y1, width) => painter.line(
                self.at(x0, y0),
                self.at(x1, y1),
                self.size(width) + grow * 2.0,
                color,
            ),
            Shape::Tilted(left, top, right, bottom, radius, angle) => {
                let rect = Rect::from_min_max(self.at(left, top), self.at(right, bottom));
                painter.rotated(rect.center(), angle).rect_filled(
                    rect.expand(grow),
                    self.size(radius) + grow,
                    color,
                );
            }
        }
    }

    fn silhouette(&self, painter: &Painter, shapes: &[Shape], fill: Color32, edge: Color32) {
        for shape in shapes {
            self.paint(painter, *shape, OUTLINE, edge);
        }
        for shape in shapes {
            self.paint(painter, *shape, 0.0, fill);
        }
    }
}

const BASE: Shape = Shape::Box(0.22, 0.76, 0.78, 0.86, 0.03);

fn chess_shapes(kind: &str) -> Option<Vec<Shape>> {
    Some(match kind {
        "pawn" => vec![
            BASE,
            Shape::Box(0.34, 0.6, 0.66, 0.8, 0.06),
            Shape::Box(0.42, 0.42, 0.58, 0.64, 0.04),
            Shape::Disc(0.5, 0.33, 0.13),
        ],
        "rook" => vec![
            BASE,
            Shape::Box(0.32, 0.36, 0.68, 0.8, 0.02),
            Shape::Box(0.26, 0.27, 0.74, 0.38, 0.02),
            Shape::Box(0.26, 0.16, 0.36, 0.3, 0.01),
            Shape::Box(0.45, 0.16, 0.55, 0.3, 0.01),
            Shape::Box(0.64, 0.16, 0.74, 0.3, 0.01),
        ],
        "knight" => vec![
            BASE,
            Shape::Box(0.4, 0.36, 0.7, 0.8, 0.08),
            Shape::Tilted(0.22, 0.3, 0.62, 0.48, 0.08, -0.45),
            Shape::Box(0.52, 0.14, 0.64, 0.32, 0.04),
        ],
        "bishop" => vec![
            BASE,
            Shape::Box(0.34, 0.64, 0.66, 0.8, 0.04),
            Shape::Box(0.36, 0.26, 0.64, 0.68, 0.14),
            Shape::Disc(0.5, 0.17, 0.06),
        ],
        "queen" => vec![
            BASE,
            Shape::Box(0.32, 0.44, 0.68, 0.8, 0.04),
            Shape::Box(0.26, 0.38, 0.74, 0.48, 0.03),
            Shape::Bar(0.24, 0.26, 0.36, 0.42, 0.07),
            Shape::Bar(0.37, 0.22, 0.43, 0.42, 0.07),
            Shape::Bar(0.5, 0.18, 0.5, 0.42, 0.07),
            Shape::Bar(0.63, 0.22, 0.57, 0.42, 0.07),
            Shape::Bar(0.76, 0.26, 0.64, 0.42, 0.07),
            Shape::Disc(0.24, 0.25, 0.05),
            Shape::Disc(0.37, 0.21, 0.05),
            Shape::Disc(0.5, 0.17, 0.05),
            Shape::Disc(0.63, 0.21, 0.05),
            Shape::Disc(0.76, 0.25, 0.05),
        ],
        "king" => vec![
            BASE,
            Shape::Box(0.32, 0.44, 0.68, 0.8, 0.04),
            Shape::Box(0.28, 0.28, 0.72, 0.48, 0.1),
            Shape::Box(0.46, 0.06, 0.54, 0.3, 0.01),
            Shape::Box(0.38, 0.12, 0.62, 0.2, 0.01),
        ],
        _ => return None,
    })
}

fn paint_piece(painter: &Painter, rect: Rect, piece: &Piece) {
    let unit = Unit { rect };
    let (fill, edge) = match piece.seat % 2 {
        0 => (IVORY, IVORY_EDGE),
        _ => (EBONY, EBONY_EDGE),
    };
    if let Some(shapes) = chess_shapes(&piece.kind) {
        unit.silhouette(painter, &shapes, fill, edge);
        match piece.kind.as_str() {
            "knight" => unit.paint(painter, Shape::Disc(0.46, 0.33, 0.028), 0.0, edge),
            "bishop" => unit.paint(painter, Shape::Bar(0.44, 0.36, 0.56, 0.48, 0.04), 0.0, edge),
            _ => {}
        }
        return;
    }
    let color = SEATS[piece.seat as usize % SEATS.len()];
    match piece.kind.as_str() {
        "man" | "crowned" => {
            unit.silhouette(painter, &[Shape::Disc(0.5, 0.5, 0.36)], fill, edge);
            unit.paint(painter, Shape::Disc(0.5, 0.5, 0.25), 0.0, edge);
            unit.paint(painter, Shape::Disc(0.5, 0.5, 0.25), -0.035, fill);
            if piece.kind == "crowned" {
                for shape in [
                    Shape::Box(0.36, 0.5, 0.64, 0.58, 0.01),
                    Shape::Bar(0.38, 0.52, 0.36, 0.4, 0.05),
                    Shape::Bar(0.5, 0.52, 0.5, 0.38, 0.05),
                    Shape::Bar(0.62, 0.52, 0.64, 0.4, 0.05),
                ] {
                    unit.paint(painter, shape, 0.0, GOLD);
                }
            }
        }
        "x" => {
            unit.paint(painter, Shape::Bar(0.2, 0.2, 0.8, 0.8, 0.1), 0.0, color);
            unit.paint(painter, Shape::Bar(0.8, 0.2, 0.2, 0.8, 0.1), 0.0, color);
        }
        "o" => {
            let ring = rect.shrink(unit.size(0.2));
            painter.rect_stroke(ring, ring.width() / 2.0, unit.size(0.1), color);
        }
        "disc" => unit.paint(painter, Shape::Disc(0.5, 0.5, 0.36), 0.0, color),
        kind => {
            let face = rect.shrink(unit.size(0.18));
            painter.rect_filled(face, unit.size(0.06), color);
            centered(painter, face, kind, unit.size(0.3), Color32::WHITE);
        }
    }
}

fn centered(painter: &Painter, rect: Rect, text: &str, size: f32, color: Color32) {
    let galley = painter.layout(text, FontId::proportional(size), f32::INFINITY);
    let origin = rect.center() - galley.size() / 2.0;
    painter.galley(origin, galley, color);
}

fn paint_card(painter: &Painter, rect: Rect, card: Card) {
    let unit = rect.width();
    let radius = unit * 0.09;
    painter.rect_filled(rect, radius, CARD_FACE);
    painter.rect_stroke(rect, radius, 1.0, CARD_EDGE);
    let color = suit_color(card.suit);
    let galley = painter.layout(
        rank_name(card.rank),
        FontId {
            bold: true,
            ..FontId::proportional(unit * 0.26)
        },
        f32::INFINITY,
    );
    let corner = rect.min + vec2(unit * 0.08, unit * 0.05);
    let under = corner.y + galley.size().y;
    painter.galley(corner, galley, color);
    let small = unit * 0.2;
    paint_suit(
        painter,
        Rect::from_min_size(pos2(corner.x, under), Vec2::splat(small)),
        card.suit,
        color,
    );
    let pip = unit * 0.5;
    paint_suit(
        painter,
        Rect::from_center_size(rect.center() + vec2(0.0, unit * 0.1), Vec2::splat(pip)),
        card.suit,
        color,
    );
}

fn paint_back(painter: &Painter, rect: Rect) {
    let unit = rect.width();
    painter.rect_filled(rect, unit * 0.09, CARD_BACK);
    painter.rect_stroke(rect, unit * 0.09, 1.0, CARD_EDGE);
    let trim = rect.shrink(unit * 0.09);
    painter.rect_stroke(trim, unit * 0.06, unit * 0.025, CARD_BACK_TRIM);
}

pub(crate) fn paint_suit(painter: &Painter, rect: Rect, suit: Suit, color: Color32) {
    let size = rect.width().min(rect.height());
    let at = |x: f32, y: f32| rect.min + Vec2::new(x * size, y * size);
    let circle = |x: f32, y: f32, radius: f32| {
        let radius = radius * size;
        painter.rect_filled(
            Rect::from_center_size(at(x, y), Vec2::splat(radius * 2.0)),
            radius,
            color,
        );
    };
    let diamond = |x: f32, y: f32, side: f32| {
        let center = at(x, y);
        painter.rotated(center, FRAC_PI_4).rect_filled(
            Rect::from_center_size(center, Vec2::splat(side * size)),
            0.0,
            color,
        );
    };
    let stem = || {
        painter.rect_filled(
            Rect::from_min_max(at(0.44, 0.55), at(0.56, 0.95)),
            0.0,
            color,
        );
        painter.rect_filled(
            Rect::from_min_max(at(0.3, 0.88), at(0.7, 0.97)),
            0.04 * size,
            color,
        );
    };
    match suit {
        Suit::Hearts => {
            circle(0.29, 0.34, 0.24);
            circle(0.71, 0.34, 0.24);
            diamond(0.5, 0.5, 0.49);
        }
        Suit::Diamonds => {
            painter.rotated(at(0.5, 0.5), FRAC_PI_4).rect_filled(
                Rect::from_center_size(at(0.5, 0.5), Vec2::new(0.5 * size, 0.5 * size)),
                0.02 * size,
                color,
            );
        }
        Suit::Spades => {
            circle(0.29, 0.56, 0.22);
            circle(0.71, 0.56, 0.22);
            diamond(0.5, 0.42, 0.45);
            stem();
        }
        Suit::Clubs => {
            circle(0.5, 0.26, 0.2);
            circle(0.28, 0.58, 0.2);
            circle(0.72, 0.58, 0.2);
            stem();
        }
    }
}

fn suit_color(suit: Suit) -> Color32 {
    match suit {
        Suit::Hearts | Suit::Diamonds => RED_SUIT,
        Suit::Clubs | Suit::Spades => BLACK_SUIT,
    }
}

fn rank_name(rank: Rank) -> &'static str {
    match rank {
        Rank::Two => "2",
        Rank::Three => "3",
        Rank::Four => "4",
        Rank::Five => "5",
        Rank::Six => "6",
        Rank::Seven => "7",
        Rank::Eight => "8",
        Rank::Nine => "9",
        Rank::Ten => "10",
        Rank::Jack => "J",
        Rank::Queen => "Q",
        Rank::King => "K",
        Rank::Ace => "A",
    }
}
