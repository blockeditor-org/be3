use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::datetime::{Date, SECONDS_PER_DAY, Weekday};
use block_editor_beui::beui::icons::{ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use database_types::database::{DatabaseRow, DatabaseValue};
use database_types::database_schema::DatabaseFieldType;
use uuid::Uuid;

use crate::database_view::app::data::Data;
use beui::{Align, Direction, ItemSize};

mod month;
mod timeline;

use month::MonthGrid;
use timeline::Timeline;

const HOUR: i64 = 3600;
const FIRST_DAY: Weekday = Weekday::Sunday;
const NEW_EVENT_HOUR: u8 = 9;
const GUTTER: Space = Space::Lg;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Layout {
    Day,
    Week,
    #[default]
    Month,
}

impl Layout {
    const ALL: [Self; 3] = [Self::Day, Self::Week, Self::Month];

    fn label(self) -> &'static str {
        match self {
            Self::Day => "Day",
            Self::Week => "Week",
            Self::Month => "Month",
        }
    }

    fn step(self, anchor: i64, by: i64) -> i64 {
        let anchor = Date::from_days(anchor);
        match self {
            Self::Day => anchor.add_days(by),
            Self::Week => anchor.add_days(7 * by),
            Self::Month => anchor.first_of_month().add_months(by as i32),
        }
        .days()
    }

    fn header(self, anchor: i64) -> String {
        let anchor = Date::from_days(anchor);
        match self {
            Self::Day => format!(
                "{}, {} {}, {}",
                anchor.weekday().name(),
                anchor.month_name(),
                anchor.day,
                anchor.year
            ),
            Self::Week => {
                let start = anchor.start_of_week(FIRST_DAY);
                let end = start.add_days(6);
                match start.same_month(end) {
                    true => format!(
                        "{} {} - {}, {}",
                        start.month_name(),
                        start.day,
                        end.day,
                        start.year
                    ),
                    false => format!(
                        "{} {} - {} {}, {}",
                        start.month_name(),
                        start.day,
                        end.month_name(),
                        end.day,
                        end.year
                    ),
                }
            }
            Self::Month => anchor.month_label(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct Event {
    pub(crate) row: usize,
    pub(crate) title: String,
    pub(crate) start: i64,
    pub(crate) end: i64,
}

#[component]
pub fn CalendarView(data: Data) -> NodeId {
    let ready = create_memo(move || {
        let start = data.calendar_start.get();
        data.fields.with(|fields| {
            fields.iter().any(|field| {
                Some(field.id) == start && field.field_type == DatabaseFieldType::Datetime
            })
        })
    });
    let missing = create_memo(move || !ready.get());
    view! {
        <Frame fill=ColorRole::Background>
            <List spacing=Space::None>
                <Show condition={missing}>
                    <Frame padding=GUTTER>
                        <Text style=TextStyle::CAPTION>
                            "Choose a date field to use the calendar view."
                        </Text>
                    </Frame>
                </Show>
                <Show condition={ready}>
                    <Calendar @sizing=ItemSize::Percent(100.0) data={data} />
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn Calendar(data: Data) -> NodeId {
    let events = events(data);
    let layout = create_signal(Layout::default());
    let anchor = create_signal(today());
    let header = create_memo(move || layout.get().header(anchor.get()));
    let layout_index = create_memo(move || {
        Layout::ALL
            .iter()
            .position(|shown| *shown == layout.get())
            .unwrap_or(0)
    });
    let previous = move || anchor.set(layout.get_untracked().step(anchor.get_untracked(), -1));
    let next = move || anchor.set(layout.get_untracked().step(anchor.get_untracked(), 1));
    let to_today = move || anchor.set(today());
    let pick_day = move |day: i64| add_event(data, day, NEW_EVENT_HOUR);
    let pick_slot = move |(day, hour): (i64, u8)| add_event(data, day, hour);
    let pick_event = move |row: usize| data.select(row, data.title_field.get_untracked());
    view! {
        <List spacing=Space::None>
            <Frame padding={Padding::symmetric(GUTTER, Space::Sm)}>
                <List
                    direction=Direction::Horizontal
                    align=Align::Center
                    spacing=Space::Sm
                    wrap=true
                >
                    <Button
                        icon_only=true
                        glyph=ICON_CHEVRON_LEFT
                        label="Previous"
                        @test_id={"database-view.calendar.previous"}
                        on_click={previous}
                    />
                    <Button
                        label="Today"
                        variant=ButtonVariant::Secondary
                        @test_id={"database-view.calendar.today"}
                        on_click={to_today}
                    />
                    <Button
                        icon_only=true
                        glyph=ICON_CHEVRON_RIGHT
                        label="Next"
                        @test_id={"database-view.calendar.next"}
                        on_click={next}
                    />
                    <Text>{header}</Text>
                    <SegmentedControl
                        label="Layout"
                        options={view! {
                            <ChoiceOption label={Layout::Day.label()} />
                            <ChoiceOption label={Layout::Week.label()} />
                            <ChoiceOption label={Layout::Month.label()} />
                        }}
                        selected={Binding::new(layout_index, move |chosen: usize| {
                            if let Some(shown) = Layout::ALL.get(chosen) {
                                layout.set(*shown);
                            }
                        })}
                        @test_id={"database-view.calendar.layout"}
                    />
                </List>
            </Frame>
            <Frame
                @sizing=ItemSize::Percent(100.0)
                padding={Padding::horizontal(GUTTER).bottom(GUTTER)}
            >
                <List spacing=Space::None>
                    <Dynamic value={layout}>|shown: Layout| {
                        match shown {
                            Layout::Month => view! {
                                <MonthGrid
                                    anchor={create_memo(move || anchor.get())}
                                    events={events}
                                    on_pick_day={pick_day}
                                    on_pick_event={pick_event}
                                />
                            },
                            Layout::Week => view! {
                                <Timeline
                                    first_day={create_memo(move || week_start(anchor.get()))}
                                    days=7
                                    events={events}
                                    on_pick_slot={pick_slot}
                                    on_pick_event={pick_event}
                                />
                            },
                            Layout::Day => view! {
                                <Timeline
                                    first_day={create_memo(move || anchor.get())}
                                    days=1
                                    events={events}
                                    on_pick_slot={pick_slot}
                                    on_pick_event={pick_event}
                                />
                            },
                        }
                    }</Dynamic>
                </List>
            </Frame>
        </List>
    }
}

fn add_event(data: Data, day: i64, hour: u8) {
    let Some(start) = data.calendar_start.get_untracked() else {
        return;
    };
    if data.read_only.get_untracked() {
        return;
    }
    let starts = day * SECONDS_PER_DAY + i64::from(hour) * HOUR;
    let mut values = vec![(start, DatabaseValue::Datetime(starts))];
    if let Some(end) = data.calendar_end.get_untracked() {
        values.push((end, DatabaseValue::Datetime(starts + HOUR)));
    }
    if let Some(title) = data.title_field.get_untracked() {
        values.push((title, DatabaseValue::String(String::new())));
    }
    data.new_row(values);
}

fn events(data: Data) -> Memo<Vec<Event>> {
    create_memo(move || {
        let Some(start) = data.calendar_start.get() else {
            return Vec::new();
        };
        let end = data.calendar_end.get();
        let title = data.title_field.get();
        data.groups.with(|groups| {
            data.rows.with(|rows| {
                let mut events: Vec<Event> = groups
                    .iter()
                    .flat_map(|group| group.rows.iter().copied())
                    .filter_map(|index| {
                        let row = rows.get(index)?;
                        let starts = datetime(row, start)?;
                        let ends = end
                            .and_then(|end| datetime(row, end))
                            .map_or(starts + HOUR, |ends| ends.max(starts));
                        let title = title
                            .and_then(|title| match row.value(title) {
                                Some(DatabaseValue::String(text)) if !text.trim().is_empty() => {
                                    Some(text.clone())
                                }
                                _ => None,
                            })
                            .unwrap_or_else(|| "Untitled".to_owned());
                        Some(Event {
                            row: index,
                            title,
                            start: starts,
                            end: ends,
                        })
                    })
                    .collect();
                events.sort_by_key(|event| event.start);
                events
            })
        })
    })
}

fn datetime(row: &DatabaseRow, field: Uuid) -> Option<i64> {
    match row.value(field) {
        Some(DatabaseValue::Datetime(seconds)) => Some(*seconds),
        _ => None,
    }
}

pub(crate) fn events_on(events: &[Event], day: i64) -> Vec<Event> {
    let start = day * SECONDS_PER_DAY;
    let end = start + SECONDS_PER_DAY;
    events
        .iter()
        .filter(|event| event.start < end && event.end.max(event.start + 1) > start)
        .cloned()
        .collect()
}

pub(crate) fn assign_lanes(events: &[Event]) -> (Vec<usize>, usize) {
    let mut lane_ends: Vec<i64> = Vec::new();
    let mut lanes = Vec::with_capacity(events.len());
    for event in events {
        match lane_ends.iter().position(|end| *end <= event.start) {
            Some(lane) => {
                lane_ends[lane] = event.end;
                lanes.push(lane);
            }
            None => {
                lanes.push(lane_ends.len());
                lane_ends.push(event.end);
            }
        }
    }
    (lanes, lane_ends.len().max(1))
}

pub(crate) fn week_start(day: i64) -> i64 {
    Date::from_days(day).start_of_week(FIRST_DAY).days()
}

pub(crate) fn weekday_abbreviation(day: i64) -> &'static str {
    &Date::from_days(day).weekday().name()[..3]
}

pub(crate) fn today() -> i64 {
    let seconds = i64::try_from(block_editor_beui::wall_clock().as_secs()).unwrap_or(0)
        + i64::from(block_editor_beui::utc_offset());
    seconds.div_euclid(SECONDS_PER_DAY)
}

pub(crate) fn forward<T: 'static>(callback: Callback<T>) -> impl Fn(T) + 'static {
    move |value| callback.call(value)
}
