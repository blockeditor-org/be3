use block_editor_beui::be_block::ObjectId;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::TextAlign;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_ui::datetime::{civil_from_days, days_from_civil};

use super::model::{Shown, WEEKDAY_ABBR, events_on, today_days_since_epoch, weekday_from_days};
use beui::{Direction, ItemSize};

const CELL_HEIGHT: f32 = 92.0;
const CELL_SPACING: Space = Space::Xs2;
const MAX_VISIBLE: usize = 3;
const WEEKS: i64 = 6;

#[component]
pub(crate) fn MonthGrid(
    anchor: Memo<i64>,
    events: Memo<Vec<Shown>>,
    on_pick_day: Callback<i64>,
    on_pick_event: Callback<Shown>,
) -> NodeId {
    let weeks = create_memo(move || {
        let (year, month, _) = civil_from_days(anchor.get());
        let first = days_from_civil(year, month, 1);
        let start = first - i64::from(weekday_from_days(first));
        (0..WEEKS)
            .map(|week| start + week * 7)
            .collect::<Vec<i64>>()
    });
    view! {
        <List spacing=CELL_SPACING>
            <List direction=Direction::Horizontal spacing=CELL_SPACING>
                <ForEach keys={weekday_names()}>|name: &'static str|
                    <Text @sizing=ItemSize::Percent(100.0) align=TextAlign::Center>{name}</Text>
                </ForEach>
            </List>
            <ForEach keys={weeks}>|start: i64|
                <MonthWeek
                    @sizing=ItemSize::Fixed(CELL_HEIGHT)
                    start={start}
                    anchor={anchor}
                    events={events}
                    on_pick_day={forward(on_pick_day)}
                    on_pick_event={forward(on_pick_event)}
                />
            </ForEach>
        </List>
    }
}

#[component]
fn MonthWeek(
    start: i64,
    anchor: Memo<i64>,
    events: Memo<Vec<Shown>>,
    on_pick_day: Callback<i64>,
    on_pick_event: Callback<Shown>,
) -> NodeId {
    let days = create_memo(move || (0..7).map(|offset| start + offset).collect::<Vec<i64>>());
    view! {
        <List direction=Direction::Horizontal spacing=CELL_SPACING>
            <ForEach keys={days}>|day: i64|
                <MonthCell
                    @sizing=ItemSize::Percent(100.0)
                    day={day}
                    anchor={anchor}
                    events={events}
                    on_pick_day={forward(on_pick_day)}
                    on_pick_event={forward(on_pick_event)}
                />
            </ForEach>
        </List>
    }
}

#[component]
fn MonthCell(
    day: i64,
    anchor: Memo<i64>,
    events: Memo<Vec<Shown>>,
    on_pick_day: Callback<i64>,
    on_pick_event: Callback<Shown>,
) -> NodeId {
    let (_, _, day_number) = civil_from_days(day);
    let in_month = create_memo(move || {
        let (year, month, _) = civil_from_days(anchor.get());
        let (day_year, day_month, _) = civil_from_days(day);
        day_year == year && day_month == month
    });
    let today = day == today_days_since_epoch();
    let day_events = create_memo(move || events.with(|events| events_on(events, day)));
    let shown = create_memo(move || {
        day_events.with(|events| {
            events
                .iter()
                .take(MAX_VISIBLE)
                .map(|event| event.id)
                .collect::<Vec<_>>()
        })
    });
    let overflow = create_memo(move || {
        let count = day_events.with(Vec::len);
        match count > MAX_VISIBLE {
            true => format!("+{} more", count - MAX_VISIBLE),
            false => String::new(),
        }
    });
    let fill = create_memo(move || match (today, in_month.get()) {
        (true, _) => ColorRole::AccentSoft,
        (false, true) => ColorRole::Surface,
        (false, false) => ColorRole::Background,
    });
    let number_color = create_memo(move || match in_month.get() {
        true => TextColor::Default,
        false => TextColor::Muted,
    });
    let pick = move || on_pick_day.call(day);
    view! {
        <Frame
            fill={fill}
            border={Border::new(ColorRole::Border).width(Stroke::Hairline)}
            radius=Radius::Sm
            padding=Space::Xs
        >
            <List spacing=Space::Xs2>
                <ListRow
                    @test_id={format!("calendar.day.{day}")}
                    on_click={pick}
                    on_activate={pick}
                >
                    <Text style=TextStyle::CAPTION color={number_color}>
                        {day_number.to_string()}
                    </Text>
                </ListRow>
                <ForEach keys={shown}>|id: ObjectId| {
                    let event = event_of(day_events, id);
                    let title = create_memo(move || {
                        event.get().map(|event| event.value.title).unwrap_or_default()
                    });
                    let open = move || {
                        if let Some(event) = event.get_untracked() {
                            on_pick_event.call(event);
                        }
                    };
                    view! {
                        <EventChip
                            title={title}
                            @test_id={format!("calendar.event.{id}")}
                            on_click={open}
                        />
                    }
                }</ForEach>
                <Text style=TextStyle::CAPTION>{overflow}</Text>
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
        </Frame>
    }
}

#[component]
fn EventChip(
    title: Memo<String>,
    on_click: block_editor_beui::beui::reactive::ClickCallback,
) -> NodeId {
    view! {
        <Frame fill=ColorRole::Accent radius=Radius::Xs>
            <ListRow on_click={on_click} on_activate={on_click}>
                <Text style=TextStyle::CAPTION color=TextColor::OnAccent>{title}</Text>
            </ListRow>
        </Frame>
    }
}

fn weekday_names() -> Memo<Vec<&'static str>> {
    create_memo(|| WEEKDAY_ABBR.to_vec())
}

fn event_of(events: Memo<Vec<Shown>>, id: ObjectId) -> Memo<Option<Shown>> {
    create_memo(move || events.with(|events| events.iter().find(|event| event.id == id).cloned()))
}

fn forward<T: 'static>(callback: Callback<T>) -> impl Fn(T) + 'static {
    move |value| callback.call(value)
}
