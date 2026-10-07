use std::rc::Rc;

use block_editor_beui::beui::reactive::{
    Canvas, CanvasItem, CanvasView, Draw, Drawing, ForEach, Frame, Memo, Prop, ReadSignal,
    WriteSignal, clone, component, component_rect, create_effect, create_memo, view,
};
use block_editor_beui::beui::styled::use_theme;
use block_editor_beui::beui::{NodeId, Painter, Pos2, Rect, Vec2, pos2};
use game_api::Spot;
use game_api::board::{CARD_HEIGHT, CARD_WIDTH, HandCard, ItemId};

use super::board::{paint_card_mark, spot_test_id};
use super::play::Play;
use super::skin::paint_sprite;

pub(crate) const HAND_CARD: Vec2 = Vec2::new(CARD_WIDTH as f32, CARD_HEIGHT as f32);
const PADDING: f32 = 12.0;
const STEP: f32 = HAND_CARD.x + 8.0;
pub(crate) const BAR_HEIGHT: f32 = HAND_CARD.y + PADDING * 2.0;

pub(crate) fn hand_rects(bar: Rect, count: usize) -> Vec<Rect> {
    let room = (bar.width() - PADDING * 2.0).max(HAND_CARD.x);
    let step = match count {
        0 | 1 => 0.0,
        count => ((room - HAND_CARD.x) / (count - 1) as f32).min(STEP),
    };
    let width = HAND_CARD.x + step * count.saturating_sub(1) as f32;
    let left = (bar.center().x - width / 2.0).max(bar.left() + PADDING);
    let top = bar.top() + PADDING;
    (0..count)
        .map(|card| Rect::from_min_size(pos2(left + step * card as f32, top), HAND_CARD))
        .collect()
}

pub(crate) fn hand_hit(bar: Rect, hand: &[HandCard], point: Pos2) -> Option<Spot> {
    hand_rects(bar, hand.len())
        .iter()
        .zip(hand)
        .rev()
        .find(|(rect, _)| rect.contains(point))
        .map(|(_, held)| held.spot)
}

#[component]
pub(crate) fn HandBar(
    hand: Memo<Vec<HandCard>>,
    set_bar: WriteSignal<Rect>,
    play: Play,
    hidden: Memo<Vec<ItemId>>,
) -> NodeId {
    let rect = component_rect();
    let bar = rect.clone();
    create_effect(move || set_bar.set(bar.get()));
    let theme = use_theme();
    let shown = create_memo(clone!(hand -> move || !hand.with(Vec::is_empty)));
    let height =
        create_memo(clone!(shown -> move || Some(if shown.get() { BAR_HEIGHT } else { 0.0 })));
    let keys = create_memo(clone!(hand -> move || {
        hand.with(|hand| hand.iter().map(|held| (held.id, held.spot)).collect::<Vec<_>>())
    }));
    let screen = Some(CanvasView::new(Pos2::ZERO, 1.0));
    view! {
        <Frame
            height={height}
            color={theme.surface.clone()}
            outline={theme.border.clone()}
            outline_width=1.0
            outline_visible=true
            visible={shown}
            @test_id={"game.hand"}
        >
            <Canvas view={screen}>
                <ForEach keys={keys}>
                    {move |(id, spot): (ItemId, Spot)| view! {
                        <HandItem
                            id
                            spot
                            hand={hand.clone()}
                            bar={rect.clone()}
                            play={play.clone()}
                            hidden={hidden.clone()}
                        />
                    }}
                </ForEach>
            </Canvas>
        </Frame>
    }
}

#[component]
fn HandItem(
    id: ItemId,
    spot: Spot,
    hand: Memo<Vec<HandCard>>,
    bar: ReadSignal<Rect>,
    play: Play,
    hidden: Memo<Vec<ItemId>>,
) -> CanvasItem {
    let held = create_memo(clone!(hand -> move || {
        hand.with(|hand| {
            let index = hand.iter().position(|held| held.id == id)?;
            Some((index, hand.len(), hand[index].clone()))
        })
    }));
    let rect = create_memo(clone!(held -> move || {
        held.get()
            .and_then(|(index, count, _)| hand_rects(bar.get(), count).get(index).copied())
            .unwrap_or(Rect::ZERO)
    }));
    let theme = use_theme();
    let draw: Prop<Draw> = Prop::Dynamic(Rc::new(clone!(held -> move || {
        let held = held.get();
        let mark = play.mark(spot);
        let hidden = hidden.with(|hidden| hidden.contains(&id));
        let theme = theme.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            let Some((_, _, held)) = &held else {
                return;
            };
            if !hidden {
                paint_sprite(painter, rect, &held.sprite);
            }
            paint_card_mark(painter, rect, mark, 1.0, &theme);
        }) as Draw
    })));
    view! {
        <CanvasItem
            x={create_memo(clone!(rect -> move || rect.get().left()))}
            y={create_memo(clone!(rect -> move || rect.get().top()))}
            width={create_memo(clone!(rect -> move || rect.get().width().max(1.0)))}
            height={create_memo(clone!(rect -> move || rect.get().height().max(1.0)))}
            clip=false
            @test_id={spot_test_id(spot)}
        >
            <Drawing draw={draw} />
        </CanvasItem>
    }
}
