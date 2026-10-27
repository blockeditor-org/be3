use block_editor_beui::beui::CanvasView;
use std::rc::Rc;

use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, Pos2, Rect, Vec2, pos2, reactive::Painter};
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
    bar: Signal<Rect>,
    play: Play,
    hidden: Memo<Vec<ItemId>>,
) -> NodeId {
    let rect_held = create_signal(Rect::ZERO);
    let rect = rect_held.read_only();
    create_effect(move || bar.set(rect.get()));
    let shown = create_memo(move || !hand.with(Vec::is_empty));
    let keys = create_memo(move || {
        hand.with(|hand| {
            hand.iter()
                .map(|held| (held.id, held.spot))
                .collect::<Vec<_>>()
        })
    });
    let screen = Some(CanvasView::new(Pos2::ZERO, 1.0));
    let built = view! {
        <Layers>
            <Show condition={shown}>
                <Frame
                    height={Size::Custom(BAR_HEIGHT)}
                    fill=ColorRole::Surface
                    border={Border::new(ColorRole::Border).width(Stroke::Hairline)}
                    @test_id={"game.hand"}
                >
                    <Canvas view={screen}>
                        <ForEach keys={keys}>|(id, spot): (ItemId, Spot)|
                            <HandItem id spot hand={hand} bar={rect} play={play} hidden={hidden} />
                        </ForEach>
                    </Canvas>
                </Frame>
            </Show>
        </Layers>
    };
    let rect_measured = measure(built);
    create_effect(move || {
        if let Some(measured) = rect_measured.get() {
            rect_held.set(measured.rect);
        }
    });
    built
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
    let held = create_memo(move || {
        hand.with(|hand| {
            let index = hand.iter().position(|held| held.id == id)?;
            Some((index, hand.len(), hand[index].clone()))
        })
    });
    let rect = create_memo(move || {
        held.get()
            .and_then(|(index, count, _)| hand_rects(bar.get(), count).get(index).copied())
            .unwrap_or(Rect::ZERO)
    });
    let theme = use_theme();
    let draw: Prop<Draw> = Prop::dynamic(move || {
        let held = held.get();
        let mark = play.mark(spot);
        let hidden = hidden.with(|hidden| hidden.contains(&id));
        let theme = theme.get().resolve();
        Rc::new(move |painter: &Painter, rect: Rect| {
            let Some((_, _, held)) = &held else {
                return;
            };
            if !hidden {
                paint_sprite(painter, rect, &held.sprite);
            }
            paint_card_mark(painter, rect, mark, 1.0, &theme);
        }) as Draw
    });
    view! {
        <CanvasItem
            x={create_memo(move || rect.get().left())}
            y={create_memo(move || rect.get().top())}
            width={create_memo(move || rect.get().width().max(1.0))}
            height={create_memo(move || rect.get().height().max(1.0))}
            clip=false
            @test_id={spot_test_id(spot)}
        >
            <Drawing draw={draw} />
        </CanvasItem>
    }
}
