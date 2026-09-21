use block_client::blocks::calendar::CalendarEvent;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::TextAlign;
use block_editor_plugin::beui::reactive::{
    Callback, Canvas, CanvasItem, ClickCatcher, Direction, ForEach, Frame, ItemSize, List, Memo,
    clone, component, component_rect, component_size, create_memo, view,
};
use block_editor_plugin::beui::styled::{Body, Caption, ListRow, Scroll, use_theme};
use block_ui::datetime::civil_from_days;
use uuid::Uuid;

use super::model::{
    SECONDS_PER_DAY, WEEKDAY_ABBR, assign_lanes, events_on, today_days_since_epoch,
    weekday_from_days,
};

const HEADER_HEIGHT: f32 = 22.0;
const HOUR_HEIGHT: f32 = 40.0;
const GUTTER: f32 = 52.0;
const CONTENT_HEIGHT: f32 = HOUR_HEIGHT * 24.0;
const MIN_EVENT_HEIGHT: f32 = 16.0;
const MIN_COLUMN_WIDTH: f32 = 90.0;
const RULE: f32 = 1.0;

#[derive(Clone, PartialEq)]
struct Placed {
    key: (Uuid, i64),
    event: CalendarEvent,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[component]
pub(crate) fn Timeline(
    first_day: Memo<i64>,
    days: usize,
    events: Memo<Vec<CalendarEvent>>,
    on_pick_slot: Callback<(i64, u8)>,
    on_pick_event: Callback<CalendarEvent>,
) -> NodeId {
    let columns = create_memo(clone!(first_day -> move || {
        (0..days as i64).map(|offset| first_day.get() + offset).collect::<Vec<i64>>()
    }));
    let theme = use_theme();
    view! {
        <List spacing=0.0>
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame width=GUTTER />
                <ForEach keys={columns}>
                    {move |day: i64| {
                        view! {
                            <DayHeader @sizing=ItemSize::Percent(100.0) day={day} />
                        }
                    }}
                </ForEach>
            </List>
            <Frame height=RULE color={theme.border.clone()} radius=0 />
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <Grid
                    first_day={first_day}
                    days={days}
                    events={events}
                    on_pick_slot={move |slot| on_pick_slot.call(slot)}
                    on_pick_event={move |event| on_pick_event.call(event)}
                />
            </Scroll>
        </List>
    }
}

#[component]
fn DayHeader(day: i64) -> NodeId {
    let (_, _, number) = civil_from_days(day);
    let label = format!("{} {number}", WEEKDAY_ABBR[weekday_from_days(day) as usize]);
    let theme = use_theme();
    let color = match day == today_days_since_epoch() {
        true => theme.accent.clone(),
        false => theme.text.clone(),
    };
    view! {
        <Frame height=HEADER_HEIGHT>
            <Body content={label} align=TextAlign::Center color={color} />
        </Frame>
    }
}

#[component]
fn Grid(
    first_day: Memo<i64>,
    days: usize,
    events: Memo<Vec<CalendarEvent>>,
    on_pick_slot: Callback<(i64, u8)>,
    on_pick_event: Callback<CalendarEvent>,
) -> NodeId {
    let size = component_size();
    let rect = component_rect();
    let column_width = create_memo(clone!(size -> move || {
        ((size.get().x - GUTTER) / days as f32).max(MIN_COLUMN_WIDTH)
    }));
    let hours = create_memo(|| (0..=24u8).collect::<Vec<u8>>());
    let rules = create_memo(move || (0..=days).collect::<Vec<usize>>());
    let placed = placements(first_day.clone(), days, events, column_width.clone());
    let keys = create_memo(clone!(placed -> move || {
        placed.with(|placed| placed.iter().map(|item| item.key).collect::<Vec<_>>())
    }));
    let slot = clone!(column_width first_day rect on_pick_slot -> move |at: block_editor_plugin::beui::Pos2| {
        let origin = rect.get_untracked().min;
        let x = at.x - origin.x - GUTTER;
        if x < 0.0 {
            return;
        }
        let column = (x / column_width.get_untracked()).floor() as i64;
        let hour = ((at.y - origin.y) / HOUR_HEIGHT).floor().clamp(0.0, 23.0) as u8;
        let day = first_day.get_untracked() + column.min(days as i64 - 1);
        on_pick_slot.call((day, hour));
    });
    let theme = use_theme();
    let hour_border = theme.border.clone();
    let rule_border = theme.border.clone();
    let rule_width = column_width.clone();
    view! {
        <Frame height=CONTENT_HEIGHT>
            <ClickCatcher on_click_at={move |press| slot(press.pos)}>
                <Canvas>
                    <ForEach keys={hours.clone()}>
                        {move |hour: u8| {
                            let top = f32::from(hour) * HOUR_HEIGHT;
                            let color = hour_border.clone();
                            view! {
                                <CanvasItem x=0.0 y={top} width=10000.0 height=RULE>
                                    <Frame color={color} radius=0 />
                                </CanvasItem>
                            }
                        }}
                    </ForEach>
                    <ForEach keys={hours}>
                        {move |hour: u8| {
                            let top = f32::from(hour) * HOUR_HEIGHT + 2.0;
                            let label = match hour < 24 {
                                true => format!("{hour:02}:00"),
                                false => String::new(),
                            };
                            view! {
                                <CanvasItem x=4.0 y={top} width=GUTTER height=16.0>
                                    <Caption content={label} />
                                </CanvasItem>
                            }
                        }}
                    </ForEach>
                    <ForEach keys={rules}>
                        {move |index: usize| {
                            let width = rule_width.clone();
                            let x = create_memo(move || GUTTER + index as f32 * width.get());
                            let color = rule_border.clone();
                            view! {
                                <CanvasItem x={x} y=0.0 width=RULE height=CONTENT_HEIGHT>
                                    <Frame color={color} radius=0 />
                                </CanvasItem>
                            }
                        }}
                    </ForEach>
                    <ForEach keys={keys}>
                        {move |key: (Uuid, i64)| {
                            let item = item_of(placed.clone(), key);
                            view! {
                                <EventBlock item={item} on_pick={forward(on_pick_event.clone())} />
                            }
                        }}
                    </ForEach>
                </Canvas>
            </ClickCatcher>
        </Frame>
    }
}

#[component]
fn EventBlock(item: Memo<Option<Placed>>, on_pick: Callback<CalendarEvent>) -> CanvasItem {
    let x = create_memo(clone!(item -> move || item.get().map_or(0.0, |item| item.x)));
    let y = create_memo(clone!(item -> move || item.get().map_or(0.0, |item| item.y)));
    let width = create_memo(clone!(item -> move || item.get().map_or(0.0, |item| item.width)));
    let height = create_memo(clone!(item -> move || item.get().map_or(0.0, |item| item.height)));
    let title = create_memo(clone!(item -> move || {
        item.get().map(|item| item.event.title).unwrap_or_default()
    }));
    let test_id = item.get_untracked().map_or_else(String::new, |item| {
        format!("calendar.event.{}", item.event.id)
    });
    let open = clone!(item on_pick -> move || {
        if let Some(item) = item.get_untracked() {
            on_pick.call(item.event);
        }
    });
    let theme = use_theme();
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <Frame color={theme.accent.clone()} radius=3>
                <ListRow @test_id={test_id} on_click={open.clone()} on_activate={open}>
                    <Caption content={title} color={theme.on_accent.clone()} />
                </ListRow>
            </Frame>
        </CanvasItem>
    }
}

fn placements(
    first_day: Memo<i64>,
    days: usize,
    events: Memo<Vec<CalendarEvent>>,
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

fn item_of(placed: Memo<Vec<Placed>>, key: (Uuid, i64)) -> Memo<Option<Placed>> {
    create_memo(move || placed.with(|placed| placed.iter().find(|item| item.key == key).cloned()))
}

fn fraction(seconds: i64) -> f32 {
    (seconds as f32 / SECONDS_PER_DAY as f32).clamp(0.0, 1.0)
}

fn forward<T: 'static>(callback: Callback<T>) -> impl Fn(T) + 'static {
    move |value| callback.call(value)
}
