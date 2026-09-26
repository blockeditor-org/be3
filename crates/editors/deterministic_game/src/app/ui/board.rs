use block_editor_plugin::beui::reactive::{
    Align, Canvas, CanvasItem, Direction, ForEach, Frame, Func, Keyed, List, Memo, ReadSignal,
    Render, Show, clone, component, create_memo, view,
};
use block_editor_plugin::beui::styled::{Caption, use_theme};
use block_editor_plugin::beui::unstyled::{
    DragHandle, DragPoint, Draggable, DropHandle, DropTarget,
};
use block_editor_plugin::beui::{Color32, NodeId};
use game_api::board::{CardTable, Grid, Pile, PilePlace, Spread, Sprite, Tile};
use game_api::{Board as Layout, Spot};

use super::play::{Mark, Play};
use super::skin::{CARD_HEIGHT, CARD_WIDTH, SpriteView};

const BOARD_SIZE: f32 = 336.0;
const MAX_TILE: f32 = 96.0;
const TILE_GAP: f32 = 4.0;
const TILE_RADIUS: u8 = 6;
const PILE_SPACING: f32 = 20.0;
const HAND_WIDTH: f32 = 440.0;
const OPPONENT_WIDTH: f32 = 180.0;
const FAN_STEP: f32 = 30.0;
const MARK_WIDTH: f32 = 3.0;
const DRAG_THRESHOLD: f32 = 6.0;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Kind {
    Empty,
    Grid(u32, u32),
    Cards,
}

fn kind(board: &Layout) -> Kind {
    match board {
        Layout::Empty => Kind::Empty,
        Layout::Grid(grid) => Kind::Grid(grid.columns, grid.rows),
        Layout::Cards(_) => Kind::Cards,
    }
}

#[component]
pub(crate) fn Board(play: Play, board: Memo<Layout>) -> NodeId {
    view! {
        <List spacing=0.0 align=Align::Start>
            <Keyed value={board} key={|board: Layout| kind(&board)}>
                {move |board: ReadSignal<Layout>| {
                    let play = play.clone();
                    match kind(&board.get_untracked()) {
                        Kind::Empty => view! {
                            <Frame />
                        },
                        Kind::Grid(columns, rows) => {
                            let grid = create_memo(move || match board.get() {
                                Layout::Grid(grid) => grid,
                                _ => Grid::default(),
                            });
                            view! {
                                <GridBoard play grid columns rows />
                            }
                        }
                        Kind::Cards => {
                            let table = create_memo(move || match board.get() {
                                Layout::Cards(table) => table,
                                _ => CardTable::default(),
                            });
                            view! {
                                <CardsBoard play table />
                            }
                        }
                    }
                }}
            </Keyed>
        </List>
    }
}

#[component]
fn GridBoard(play: Play, grid: Memo<Grid>, columns: u32, rows: u32) -> NodeId {
    let size = ((BOARD_SIZE - TILE_GAP * (columns.max(rows) as f32 - 1.0))
        / columns.max(rows).max(1) as f32)
        .min(MAX_TILE);
    let row_keys: Vec<u32> = (0..rows).collect();
    let column_keys: Vec<u32> = (0..columns).collect();
    let theme = use_theme();
    view! {
        <Frame
            color={theme.surface.clone()}
            radius=10
            padding_horizontal=TILE_GAP
            padding_vertical=TILE_GAP
            @test_id={"game.grid"}
        >
            <List spacing=TILE_GAP>
                <ForEach keys={row_keys}>
                    {move |row: u32| {
                        let play = play.clone();
                        let grid = grid.clone();
                        let column_keys = column_keys.clone();
                        view! {
                            <List direction=Direction::Horizontal spacing=TILE_GAP>
                                <ForEach keys={column_keys}>
                                    {move |column: u32| {
                                        let play = play.clone();
                                        let tile = create_memo(clone!(grid -> move || {
                                            grid.with(|grid| grid.tile(column, row).clone())
                                        }));
                                        view! {
                                            <TileView play tile column row size />
                                        }
                                    }}
                                </ForEach>
                            </List>
                        }
                    }}
                </ForEach>
            </List>
        </Frame>
    }
}

#[component]
fn TileView(play: Play, tile: Memo<Tile>, column: u32, row: u32, size: f32) -> NodeId {
    let layers = create_memo(
        clone!(tile -> move || (0..tile.with(|tile| tile.layers.len())).collect::<Vec<_>>()),
    );
    let theme = use_theme();
    view! {
        <SpotView
            play
            spot={Spot::tile(column, row)}
            radius=TILE_RADIUS
            @test_id={format!("game.tile.{column}.{row}")}
        >
            {move |mark: Memo<Mark>| {
                let fill = create_memo(clone!(theme -> move || {
                    let theme = theme.get();
                    match mark.get() {
                        Mark::Target | Mark::Over => theme.accent_soft,
                        _ => theme.surface_raised,
                    }
                }));
                view! {
                    <Frame width=size height=size color={fill} radius=TILE_RADIUS>
                        <Canvas width=size height=size>
                            <ForEach keys={layers}>
                                {move |layer: usize| {
                                    let sprite = create_memo(clone!(tile -> move || {
                                        tile.with(|tile| tile.layers.get(layer).cloned())
                                            .unwrap_or(Sprite::CardBack)
                                    }));
                                    view! {
                                        <CanvasItem x=0.0 y=0.0 width=size height=size>
                                            <SpriteView sprite size />
                                        </CanvasItem>
                                    }
                                }}
                            </ForEach>
                        </Canvas>
                    </Frame>
                }
            }}
        </SpotView>
    }
}

#[component]
fn CardsBoard(play: Play, table: Memo<CardTable>) -> NodeId {
    view! {
        <List spacing=PILE_SPACING>
            <PileRow play={play.clone()} table={table.clone()} places={&[PilePlace::Opponent]} />
            <PileRow
                play={play.clone()}
                table={table.clone()}
                places={&[PilePlace::Deck, PilePlace::Discard, PilePlace::Extra]}
            />
            <PileRow play table places={&[PilePlace::Hand]} />
        </List>
    }
}

#[component]
fn PileRow(play: Play, table: Memo<CardTable>, places: &'static [PilePlace]) -> NodeId {
    let keys = create_memo(clone!(table -> move || {
        table.with(|table| {
            table
                .piles
                .iter()
                .enumerate()
                .filter(|(_, pile)| places.contains(&pile.place))
                .map(|(index, pile)| (index as u32, pile.spread))
                .collect::<Vec<_>>()
        })
    }));
    view! {
        <List direction=Direction::Horizontal spacing=PILE_SPACING>
            <ForEach keys>
                {move |(index, spread): (u32, Spread)| {
                    let play = play.clone();
                    let pile = create_memo(clone!(table -> move || {
                        table.with(|table| table.piles.get(index as usize).cloned())
                            .unwrap_or(Pile {
                                label: String::new(),
                                place: PilePlace::Extra,
                                spread,
                                cards: Vec::new(),
                            })
                    }));
                    view! {
                        <PileView play pile index spread />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn PileView(play: Play, pile: Memo<Pile>, index: u32, spread: Spread) -> NodeId {
    let caption = create_memo(clone!(pile -> move || {
        pile.with(|pile| format!("{} ({})", pile.label, pile.cards.len()))
    }));
    let (stacked_play, stacked_pile) = (play.clone(), pile.clone());
    let stacked = spread == Spread::Stacked;
    let fanned = spread == Spread::Fanned;
    view! {
        <List spacing=6.0 align=Align::Start>
            <Caption content={caption} />
            <Show condition=stacked>
                <StackedPile play={stacked_play} pile={stacked_pile} index />
            </Show>
            <Show condition=fanned>
                <FannedPile play pile index />
            </Show>
        </List>
    }
}

#[component]
fn StackedPile(play: Play, pile: Memo<Pile>, index: u32) -> NodeId {
    let top = create_memo(clone!(pile -> move || {
        pile.with(|pile| pile.cards.last().cloned()).unwrap_or(Sprite::CardBack)
    }));
    let filled = create_memo(clone!(pile -> move || pile.with(|pile| !pile.cards.is_empty())));
    let theme = use_theme();
    view! {
        <SpotView play spot={Spot::Pile(index)} radius=6 @test_id={format!("game.pile.{index}")}>
            {move |_: Memo<Mark>| view! {
                <Frame
                    width=CARD_WIDTH
                    height=CARD_HEIGHT
                    radius=6
                    outline={theme.border.clone()}
                    outline_width=1.0
                    outline_visible=true
                >
                    <List spacing=0.0>
                        <Show condition={filled}>
                            <SpriteView sprite={top} size=CARD_WIDTH />
                        </Show>
                    </List>
                </Frame>
            }}
        </SpotView>
    }
}

#[component]
fn FannedPile(play: Play, pile: Memo<Pile>, index: u32) -> NodeId {
    let widest = match pile.with_untracked(|pile| pile.place) {
        PilePlace::Opponent => OPPONENT_WIDTH,
        _ => HAND_WIDTH,
    };
    let count = create_memo(clone!(pile -> move || pile.with(|pile| pile.cards.len())));
    let step = create_memo(clone!(count -> move || fan_step(count.get(), widest)));
    let width = create_memo(clone!(count step -> move || {
        CARD_WIDTH + step.get() * count.get().saturating_sub(1) as f32
    }));
    let keys = create_memo(clone!(count -> move || (0..count.get() as u32).collect::<Vec<_>>()));
    view! {
        <SpotView
            play={play.clone()}
            spot={Spot::Pile(index)}
            radius=6
            @test_id={format!("game.pile.{index}")}
        >
            {move |_: Memo<Mark>| view! {
                <Canvas width={width} height=CARD_HEIGHT>
                    <ForEach keys>
                        {move |card: u32| {
                            let play = play.clone();
                            let x = create_memo(clone!(step -> move || step.get() * card as f32));
                            let sprite = create_memo(clone!(pile -> move || {
                                pile.with(|pile| pile.cards.get(card as usize).cloned())
                                    .unwrap_or(Sprite::CardBack)
                            }));
                            view! {
                                <CanvasItem x width=CARD_WIDTH height=CARD_HEIGHT y=0.0>
                                    <CardView play sprite pile=index card />
                                </CanvasItem>
                            }
                        }}
                    </ForEach>
                </Canvas>
            }}
        </SpotView>
    }
}

fn fan_step(count: usize, widest: f32) -> f32 {
    if count < 2 {
        return FAN_STEP;
    }
    ((widest - CARD_WIDTH) / (count - 1) as f32).min(FAN_STEP)
}

#[component]
fn CardView(play: Play, sprite: Memo<Sprite>, pile: u32, card: u32) -> NodeId {
    view! {
        <SpotView
            play
            spot={Spot::card(pile, card)}
            radius=6
            @test_id={format!("game.card.{pile}.{card}")}
        >
            {move |_: Memo<Mark>| view! {
                <SpriteView sprite size=CARD_WIDTH />
            }}
        </SpotView>
    }
}

#[component]
fn SpotView(
    play: Play,
    spot: Spot,
    radius: u8,
    #[prop(children)] content: Render<Memo<Mark>>,
) -> NodeId {
    let payload = create_memo(clone!(play -> move || play.movable(spot).then_some(spot)));
    let accepts =
        Func::new(clone!(play -> move |from: Spot| from != spot && play.leads(from, spot)));
    let dropped = clone!(play -> move |(from, _): (Spot, DragPoint)| play.dropped(from, spot));
    let clicked = clone!(play -> move || play.click(spot));
    let preview = clone!(play -> move |from: Spot| {
        let sprite = create_memo(clone!(play -> move || {
            sprite_at(&play.board.get(), from).unwrap_or(Sprite::CardBack)
        }));
        view! {
            <SpriteView sprite size=CARD_WIDTH />
        }
    });
    let theme = use_theme();
    view! {
        <DropTarget accepts on_drop={dropped}>
            {move |handle: DropHandle| {
                let DropHandle { carrying, over, .. } = handle;
                let mark = create_memo(clone!(play -> move || {
                    if over.get() {
                        Mark::Over
                    } else if carrying.get() {
                        Mark::Target
                    } else {
                        play.mark(spot)
                    }
                }));
                let outline = create_memo(clone!(theme mark -> move || {
                    let theme = theme.get();
                    match mark.get() {
                        Mark::Selected => theme.warning,
                        Mark::Over => theme.accent_active,
                        Mark::None => Color32::TRANSPARENT,
                        Mark::Movable | Mark::Target => theme.accent,
                    }
                }));
                let marked = create_memo(clone!(mark -> move || mark.get() != Mark::None));
                let face = content.call(mark);
                view! {
                    <Draggable
                        payload
                        threshold=DRAG_THRESHOLD
                        capture_presses=true
                        preview
                        on_click={clicked}
                    >
                        {move |_: DragHandle| view! {
                            <Frame
                                radius
                                outline={outline}
                                outline_width=MARK_WIDTH
                                outline_offset=1.0
                                outline_visible={marked}
                            >
                                {face}
                            </Frame>
                        }}
                    </Draggable>
                }
            }}
        </DropTarget>
    }
}

fn sprite_at(board: &Layout, spot: Spot) -> Option<Sprite> {
    match (board, spot) {
        (Layout::Grid(grid), Spot::Tile { column, row })
            if column < grid.columns && row < grid.rows =>
        {
            grid.tile(column, row).layers.last().cloned()
        }
        (Layout::Cards(table), Spot::Card { pile, card }) => table
            .piles
            .get(pile as usize)
            .and_then(|pile| pile.cards.get(card as usize).cloned()),
        (Layout::Cards(table), Spot::Pile(pile)) => table
            .piles
            .get(pile as usize)
            .and_then(|pile| pile.cards.last().cloned()),
        _ => None,
    }
}
