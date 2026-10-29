use block_editor_beui::be_block::ObjectId;
use block_editor_beui::beui::TextAlign;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, PointerPress, Vec2};
use block_ui::datetime::civil_from_days;

use super::model::{
    SECONDS_PER_DAY, Shown, WEEKDAY_ABBR, assign_lanes, events_on, today_days_since_epoch,
    weekday_from_days,
};
use super::ui::use_form_opener;
use beui::{Direction, ItemSize};

const HEADER_HEIGHT: Size = Size::Custom(22.0);
const HOUR_HEIGHT: f32 = 40.0;
const GUTTER: f32 = 52.0;
const CONTENT_HEIGHT: f32 = HOUR_HEIGHT * 24.0;
const MIN_EVENT_HEIGHT: f32 = 16.0;
const MIN_COLUMN_WIDTH: f32 = 90.0;
const RULE: f32 = 1.0;

#[derive(Clone, PartialEq)]
struct Placed {
    key: (ObjectId, i64),
    event: Shown,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[component]
pub(crate) fn Timeline(
    first_day: Memo<i64>,
    days: usize,
    events: Memo<Vec<Shown>>,
    on_pick_slot: Callback<(i64, u8)>,
    on_pick_event: Callback<Shown>,
) -> NodeId {
    let columns = create_memo(move || {
        (0..days as i64)
            .map(|offset| first_day.get() + offset)
            .collect::<Vec<i64>>()
    });
    view! {
        <List spacing=Space::None>
            <List direction=Direction::Horizontal spacing=Space::None>
                <Frame width={Size::Custom(GUTTER)} />
                <ForEach keys={columns}>|day: i64|
                    <DayHeader @sizing=ItemSize::Percent(100.0) day={day} />
                </ForEach>
            </List>
            <Separator />
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <Grid
                    first_day={first_day}
                    days={days}
                    events={events}
                    on_pick_slot={on_pick_slot}
                    on_pick_event={on_pick_event}
                />
            </Scroll>
        </List>
    }
}

#[component]
fn DayHeader(day: i64) -> NodeId {
    let (_, _, number) = civil_from_days(day);
    let label = format!("{} {number}", WEEKDAY_ABBR[weekday_from_days(day) as usize]);
    let color = match day == today_days_since_epoch() {
        true => TextColor::Accent,
        false => TextColor::Default,
    };
    view! {
        <Frame height=HEADER_HEIGHT>
            <Text align=TextAlign::Center color={color}>{label}</Text>
        </Frame>
    }
}

#[component]
fn Grid(
    first_day: Memo<i64>,
    days: usize,
    events: Memo<Vec<Shown>>,
    on_pick_slot: Callback<(i64, u8)>,
    on_pick_event: Callback<Shown>,
) -> NodeId {
    let size_held = create_signal(Vec2::ZERO);
    let size = size_held.read_only();
    let column_width =
        create_memo(move || ((size.get().x - GUTTER) / days as f32).max(MIN_COLUMN_WIDTH));
    let hours = create_memo(|| (0..=24u8).collect::<Vec<u8>>());
    let rules = create_memo(move || (0..=days).collect::<Vec<usize>>());
    let placed = placements(first_day, days, events, column_width);
    let keys = create_memo(move || {
        placed.with(|placed| placed.iter().map(|item| item.key).collect::<Vec<_>>())
    });
    let picked_held = create_signal((0usize, 0u8));
    let picked = picked_held.read_only();
    let slot_marker = NodeRef::new();
    let opened_from = use_form_opener();
    let slot = move |at: block_editor_beui::beui::Pos2| {
        let x = at.x - GUTTER;
        if x < 0.0 {
            return;
        }
        let column = ((x / column_width.get_untracked()).floor() as usize).min(days - 1);
        let hour = (at.y / HOUR_HEIGHT).floor().clamp(0.0, 23.0) as u8;
        let day = first_day.get_untracked() + column as i64;
        picked_held.set((column, hour));
        opened_from(slot_marker);
        on_pick_slot.call((day, hour));
    };
    let slot_x = create_memo(move || GUTTER + picked.get().0 as f32 * column_width.get());
    let slot_y = create_memo(move || f32::from(picked.get().1) * HOUR_HEIGHT);
    let built = view! {
        <Frame height={Size::Custom(CONTENT_HEIGHT)}>
            <Interactive on_click_at={move |press: PointerPress| slot(press.local)}>
                <Canvas>
                    <ForEach keys={hours}>|hour: u8| {
                        let top = f32::from(hour) * HOUR_HEIGHT;
                        view! {
                            <CanvasItem x=0.0 y={top} width=10000.0 height=RULE>
                                <Frame fill=ColorRole::Border />
                            </CanvasItem>
                        }
                    }</ForEach>
                    <ForEach keys={hours}>|hour: u8| {
                        let top = f32::from(hour) * HOUR_HEIGHT + 2.0;
                        let label = match hour < 24 {
                            true => format!("{hour:02}:00"),
                            false => String::new(),
                        };
                        view! {
                            <CanvasItem x=4.0 y={top} width=GUTTER height=16.0>
                                <Text style=TextStyle::CAPTION>{label}</Text>
                            </CanvasItem>
                        }
                    }</ForEach>
                    <ForEach keys={rules}>|index: usize| {
                        let x = create_memo(move || GUTTER + index as f32 * column_width.get());
                        view! {
                            <CanvasItem x={x} y=0.0 width=RULE height=CONTENT_HEIGHT>
                                <Frame fill=ColorRole::Border />
                            </CanvasItem>
                        }
                    }</ForEach>
                    <CanvasItem x={slot_x} y={slot_y} width={column_width} height=HOUR_HEIGHT>
                        <Frame @node_ref={&slot_marker} />
                    </CanvasItem>
                    <ForEach keys={keys}>|key: (ObjectId, i64)| {
                        let item = item_of(placed, key);
                        view! {
                            <EventBlock item={item} on_pick={forward(on_pick_event)} />
                        }
                    }</ForEach>
                </Canvas>
            </Interactive>
        </Frame>
    };
    let size_measured = measure(built);
    create_effect(move || {
        if let Some(measured) = size_measured.get() {
            size_held.set(measured.size);
        }
    });
    built
}

#[component]
fn EventBlock(item: Memo<Option<Placed>>, on_pick: Callback<Shown>) -> CanvasItem {
    let x = create_memo(move || item.get().map_or(0.0, |item| item.x));
    let y = create_memo(move || item.get().map_or(0.0, |item| item.y));
    let width = create_memo(move || item.get().map_or(0.0, |item| item.width));
    let height = create_memo(move || item.get().map_or(0.0, |item| item.height));
    let title = create_memo(move || {
        item.get()
            .map(|item| item.event.value.title)
            .unwrap_or_default()
    });
    let element_test_id = item.get_untracked().map_or_else(String::new, |item| {
        format!("calendar.event.{}", item.event.id)
    });
    let opened_from = use_form_opener();
    let block = NodeRef::new();
    let open = move || {
        if let Some(item) = item.get_untracked() {
            opened_from(block);
            on_pick.call(item.event);
        }
    };
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <Frame @node_ref={&block} fill=ColorRole::Accent radius=Radius::Xs>
                <ListRow @test_id={element_test_id} on_click={open} on_activate={open}>
                    <Text style=TextStyle::CAPTION color=TextColor::OnAccent>{title}</Text>
                </ListRow>
            </Frame>
        </CanvasItem>
    }
}

fn placements(
    first_day: Memo<i64>,
    days: usize,
    events: Memo<Vec<Shown>>,
    column_width: Memo<f32>,
) -> Memo<Vec<Placed>> {
    create_memo(move || {
        let first = first_day.get();
        let width = column_width.get();
        let mut placed = Vec::new();
        for offset in 0..days as i64 {
            let day = first + offset;
            let day_start = day * SECONDS_PER_DAY;
            let day_end = day_start + SECONDS_PER_DAY;
            let day_events = events.with(|events| events_on(events, day));
            let (lanes, lane_count) = assign_lanes(&day_events);
            let column = GUTTER + offset as f32 * width;
            let lane_width = (width - 2.0) / lane_count as f32;
            for (event, lane) in day_events.into_iter().zip(lanes) {
                let start = fraction(event.start.max(day_start) - day_start);
                let end = fraction(event.end.min(day_end) - day_start);
                let top = start * CONTENT_HEIGHT;
                let bottom = (end * CONTENT_HEIGHT).max(top + MIN_EVENT_HEIGHT);
                placed.push(Placed {
                    key: (event.id, day),
                    event,
                    x: column + 1.0 + lane as f32 * lane_width,
                    y: top + 1.0,
                    width: (lane_width - 2.0).max(1.0),
                    height: (bottom - top - 2.0).max(MIN_EVENT_HEIGHT),
                });
            }
        }
        placed
    })
}

fn item_of(placed: Memo<Vec<Placed>>, key: (ObjectId, i64)) -> Memo<Option<Placed>> {
    create_memo(move || placed.with(|placed| placed.iter().find(|item| item.key == key).cloned()))
}

fn fraction(seconds: i64) -> f32 {
    (seconds as f32 / SECONDS_PER_DAY as f32).clamp(0.0, 1.0)
}

fn forward<T: 'static>(callback: Callback<T>) -> impl Fn(T) + 'static {
    move |value| callback.call(value)
}
