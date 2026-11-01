use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::TextAlign;
use block_editor_beui::beui::datetime::Date;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;

use super::{Event, FIRST_DAY, events_on, forward, today};
use beui::{Direction, ItemSize};

const CELL_HEIGHT: f32 = 92.0;
const CELL_SPACING: Space = Space::Xs2;
const MAX_VISIBLE: usize = 3;
const WEEKS: i64 = 6;

#[component]
pub(super) fn MonthGrid(
    anchor: Memo<i64>,
    events: Memo<Vec<Event>>,
    on_pick_day: Callback<i64>,
    on_pick_event: Callback<usize>,
) -> NodeId {
    let weeks = create_memo(move || {
        let start = Date::from_days(anchor.get())
            .first_of_month()
            .start_of_week(FIRST_DAY)
            .days();
        (0..WEEKS)
            .map(|week| start + week * 7)
            .collect::<Vec<i64>>()
    });
    let names = create_memo(|| {
        (0..7)
            .map(|offset| &FIRST_DAY.offset(offset).name()[..3])
            .collect::<Vec<&'static str>>()
    });
    view! {
        <List spacing=CELL_SPACING>
            <List direction=Direction::Horizontal spacing=CELL_SPACING>
                <ForEach keys={names}>|name: &'static str|
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
    events: Memo<Vec<Event>>,
    on_pick_day: Callback<i64>,
    on_pick_event: Callback<usize>,
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
    events: Memo<Vec<Event>>,
    on_pick_day: Callback<i64>,
    on_pick_event: Callback<usize>,
) -> NodeId {
    let date = Date::from_days(day);
    let in_month = create_memo(move || Date::from_days(anchor.get()).same_month(date));
    let today = day == today();
    let day_events = create_memo(move || events.with(|events| events_on(events, day)));
    let shown = create_memo(move || {
        day_events.with(|events| {
            events
                .iter()
                .take(MAX_VISIBLE)
                .map(|event| event.row)
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
                    @test_id={format!("database-view.calendar.day.{day}")}
                    on_click={pick}
                    on_activate={pick}
                >
                    <Text style=TextStyle::CAPTION color={number_color}>
                        {date.day.to_string()}
                    </Text>
                </ListRow>
                <ForEach keys={shown}>|row: usize| {
                    let title = create_memo(move || {
                        day_events.with(|events| {
                            events
                                .iter()
                                .find(|event| event.row == row)
                                .map(|event| event.title.clone())
                                .unwrap_or_default()
                        })
                    });
                    let open = move || on_pick_event.call(row);
                    view! {
                        <Frame fill=ColorRole::Accent radius=Radius::Xs>
                            <ListRow
                                @test_id={format!("database-view.calendar.event.{row}")}
                                on_click={open}
                                on_activate={open}
                            >
                                <Text style=TextStyle::CAPTION color=TextColor::OnAccent>
                                    {title}
                                </Text>
                            </ListRow>
                        </Frame>
                    }
                }</ForEach>
                <Text style=TextStyle::CAPTION>{overflow}</Text>
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
        </Frame>
    }
}
