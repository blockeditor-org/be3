use std::f32::consts::FRAC_PI_4;
use std::rc::Rc;

use block_editor_beui::beui::reactive::{
    Canvas, CanvasItem, Draw, Drawing, Dynamic, Frame, List, Memo, Stroke, Text, component, view,
};
use block_editor_beui::beui::{Color32, NodeId, Painter, Pos2, Rect, TextAlign, Vec2};
use game_api::board::{Piece, Shade, Sprite};
use game_api::cards::{Card, Rank, Suit};

pub(crate) const CARD_WIDTH: f32 = 56.0;
pub(crate) const CARD_HEIGHT: f32 = 80.0;
const CARD_RADIUS: u8 = 6;
const PIECE_INSET: f32 = 0.18;
const MARK_WIDTH: f32 = 0.1;

const SEATS: [Color32; 4] = [
    Color32::from_rgb(214, 69, 69),
    Color32::from_rgb(230, 180, 34),
    Color32::from_rgb(59, 130, 246),
    Color32::from_rgb(34, 160, 107),
];
const LIGHT_SQUARE: Color32 = Color32::from_rgb(238, 226, 200);
const DARK_SQUARE: Color32 = Color32::from_rgb(150, 111, 81);
const CARD_FACE: Color32 = Color32::from_rgb(250, 250, 247);
const CARD_EDGE: Color32 = Color32::from_rgb(190, 190, 184);
const CARD_BACK: Color32 = Color32::from_rgb(44, 82, 160);
const CARD_BACK_TRIM: Color32 = Color32::from_rgb(160, 186, 235);
const RED_SUIT: Color32 = Color32::from_rgb(200, 36, 44);
const BLACK_SUIT: Color32 = Color32::from_rgb(28, 28, 32);

pub(crate) fn seat_color(seat: u8) -> Color32 {
    SEATS[seat as usize % SEATS.len()]
}

#[component]
pub(crate) fn SpriteView(sprite: Memo<Sprite>, size: f32) -> NodeId {
    view! {
        <List spacing=0.0>
            <Dynamic value={sprite}>
                {move |sprite: Sprite| match sprite {
                    Sprite::Square(shade) => view! {
                        <Square shade size />
                    },
                    Sprite::Piece(piece) => view! {
                        <PieceView piece size />
                    },
                    Sprite::Card(card) => view! {
                        <CardFace card />
                    },
                    Sprite::CardBack => view! {
                        <CardBack />
                    },
                }}
            </Dynamic>
        </List>
    }
}

#[component]
fn Square(shade: Shade, size: f32) -> NodeId {
    let color = match shade {
        Shade::Light => LIGHT_SQUARE,
        Shade::Dark => DARK_SQUARE,
    };
    view! {
        <Frame width=size height=size color />
    }
}

#[component]
fn PieceView(piece: Piece, size: f32) -> NodeId {
    let color = seat_color(piece.seat);
    let inset = size * PIECE_INSET;
    let inner = size - inset * 2.0;
    let line = size * MARK_WIDTH;
    view! {
        <Canvas width=size height=size>
            <CanvasItem x=inset y=inset width=inner height=inner>
                <List spacing=0.0>
                    <Dynamic value={piece.kind}>
                        {move |kind: String| match kind.as_str() {
                            "x" => view! {
                                <Cross size=inner line color />
                            },
                            "o" => view! {
                                <Frame
                                    width=inner
                                    height=inner
                                    radius={(inner / 2.0) as u8}
                                    outline=color
                                    outline_width=line
                                    outline_visible=true
                                />
                            },
                            "disc" => view! {
                                <Frame
                                    width=inner
                                    height=inner
                                    radius={(inner / 2.0) as u8}
                                    color
                                />
                            },
                            _ => view! {
                                <Frame width=inner height=inner radius=4 color>
                                    <Text
                                        string={kind}
                                        font_size={inner / 2.0}
                                        color=Color32::WHITE
                                        align=TextAlign::Center
                                        bold=true
                                    />
                                </Frame>
                            },
                        }}
                    </Dynamic>
                </List>
            </CanvasItem>
        </Canvas>
    }
}

#[component]
fn Cross(size: f32, line: f32, color: Color32) -> NodeId {
    view! {
        <Canvas width=size height=size>
            <CanvasItem x=0.0 y=0.0 width=size height=size>
                <Stroke from={Pos2::new(0.0, 0.0)} to={Pos2::new(size, size)} width=line color />
            </CanvasItem>
            <CanvasItem x=0.0 y=0.0 width=size height=size>
                <Stroke from={Pos2::new(size, 0.0)} to={Pos2::new(0.0, size)} width=line color />
            </CanvasItem>
        </Canvas>
    }
}

#[component]
fn CardFace(card: Card) -> NodeId {
    let color = suit_color(card.suit);
    let rank = rank_name(card.rank).to_owned();
    let pip = CARD_WIDTH * 0.5;
    let corner = CARD_WIDTH * 0.24;
    view! {
        <Frame
            width=CARD_WIDTH
            height=CARD_HEIGHT
            color=CARD_FACE
            radius=CARD_RADIUS
            outline=CARD_EDGE
            outline_width=1.0
            outline_visible=true
        >
            <Canvas width=CARD_WIDTH height=CARD_HEIGHT>
                <CanvasItem x=4.0 y=3.0 width=corner height=20.0>
                    <Text string={rank} font_size=16.0 color bold=true align=TextAlign::Center />
                </CanvasItem>
                <CanvasItem
                    x={4.0 + corner * 0.1}
                    y=23.0
                    width={corner * 0.8}
                    height={corner * 0.8}
                >
                    <SuitMark suit={card.suit} />
                </CanvasItem>
                <CanvasItem
                    x={(CARD_WIDTH - pip) / 2.0}
                    y={(CARD_HEIGHT - pip) / 2.0 + 6.0}
                    width=pip
                    height=pip
                >
                    <SuitMark suit={card.suit} />
                </CanvasItem>
            </Canvas>
        </Frame>
    }
}

#[component]
fn CardBack() -> NodeId {
    view! {
        <Frame
            width=CARD_WIDTH
            height=CARD_HEIGHT
            color=CARD_BACK
            radius=CARD_RADIUS
            padding_horizontal=5.0
            padding_vertical=5.0
            outline=CARD_EDGE
            outline_width=1.0
            outline_visible=true
        >
            <Frame
                width={CARD_WIDTH - 10.0}
                height={CARD_HEIGHT - 10.0}
                radius={CARD_RADIUS - 2}
                outline=CARD_BACK_TRIM
                outline_width=1.5
                outline_visible=true
            />
        </Frame>
    }
}

#[component]
pub(crate) fn SuitMark(suit: Suit) -> NodeId {
    let color = suit_color(suit);
    let draw: Draw =
        Rc::new(move |painter: &Painter, rect: Rect| paint_suit(painter, rect, suit, color));
    view! {
        <Drawing draw />
    }
}

fn paint_suit(painter: &Painter, rect: Rect, suit: Suit, color: Color32) {
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
