use block_editor_beui::be_block::ObjectId;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::TextAlign;
use block_editor_beui::beui::reactive::{
    Callback, Direction, ForEach, Frame, ItemSize, List, Memo, Spacer, clone, component,
    create_memo, view,
};
use block_editor_beui::beui::styled::{Body, Caption, ListRow, use_theme};
use block_ui::datetime::{civil_from_days, days_from_civil};

use super::model::{Shown, WEEKDAY_ABBR, events_on, today_days_since_epoch, weekday_from_days};

const CELL_HEIGHT: f32 = 92.0;
const CELL_SPACING: f32 = 2.0;
const MAX_VISIBLE: usize = 3;
const WEEKS: i64 = 6;

#[component]
pub(crate) fn MonthGrid(
    anchor: Memo<i64>,
    events: Memo<Vec<Shown>>,
    on_pick_day: Callback<i64>,
    on_pick_event: Callback<Shown>,
) -> NodeId {
    let weeks = create_memo(clone!(anchor -> move || {
        let (year, month, _) = civil_from_days(anchor.get());
        let first = days_from_civil(year, month, 1);
        let start = first - i64::from(weekday_from_days(first));
        (0..WEEKS).map(|week| start + week * 7).collect::<Vec<i64>>()
    }));
    view! {
        <List spacing=CELL_SPACING>
            <List direction=Direction::Horizontal spacing=CELL_SPACING>
                <ForEach keys={weekday_names()}>
                    {move |name: &'static str| {
                        view! {
                            <Body
                                @sizing=ItemSize::Percent(100.0)
                                content={name}
                                align=TextAlign::Center
                            />
                        }
                    }}
                </ForEach>
            </List>
            <ForEach keys={weeks}>
                {move |start: i64| {
                    view! {
                        <MonthWeek
                            @sizing=ItemSize::Fixed(CELL_HEIGHT)
                            start={start}
                            anchor={anchor.clone()}
                            events={events.clone()}
                            on_pick_day={forward(on_pick_day.clone())}
                            on_pick_event={forward(on_pick_event.clone())}
                        />
                    }
                }}
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
            <ForEach keys={days}>
                {move |day: i64| {
                    view! {
                        <MonthCell
                            @sizing=ItemSize::Percent(100.0)
                            day={day}
                            anchor={anchor.clone()}
                            events={events.clone()}
                            on_pick_day={forward(on_pick_day.clone())}
                            on_pick_event={forward(on_pick_event.clone())}
                        />
                    }
                }}
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
    let in_month = create_memo(clone!(anchor -> move || {
        let (year, month, _) = civil_from_days(anchor.get());
        let (day_year, day_month, _) = civil_from_days(day);
        day_year == year && day_month == month
    }));
    let today = day == today_days_since_epoch();
    let day_events =
        create_memo(clone!(events -> move || events.with(|events| events_on(events, day))));
    let shown = create_memo(clone!(day_events -> move || {
        day_events.with(|events| {
            events.iter().take(MAX_VISIBLE).map(|event| event.id).collect::<Vec<_>>()
        })
    }));
    let overflow = create_memo(clone!(day_events -> move || {
        let count = day_events.with(Vec::len);
        match count > MAX_VISIBLE {
            true => format!("+{} more", count - MAX_VISIBLE),
            false => String::new(),
        }
    }));
    let theme = use_theme();
    let fill = create_memo(
        clone!(theme in_month -> move || match (today, in_month.get()) {
            (true, _) => theme.accent_soft.get(),
            (false, true) => theme.surface.get(),
            (false, false) => theme.background.get(),
        }),
    );
    let number_color = create_memo(clone!(theme in_month -> move || match in_month.get() {
        true => theme.text.get(),
        false => theme.text_muted.get(),
    }));
    let pick = clone!(on_pick_day -> move || on_pick_day.call(day));
    view! {
        <Frame
            color={fill}
            outline={theme.border.clone()}
            outline_width=1.0
            outline_visible=true
            radius=4
            padding_horizontal=4.0
            padding_vertical=4.0
        >
            <List spacing=2.0>
                <ListRow
                    @test_id={format!("calendar.day.{day}")}
                    on_click={pick.clone()}
                    on_activate={pick}
                >
                    <Caption content={day_number.to_string()} color={number_color} />
                </ListRow>
                <ForEach keys={shown}>
                    {move |id: ObjectId| {
                        let event = event_of(day_events.clone(), id);
                        let title = create_memo(clone!(event -> move || {
                            event.get().map(|event| event.value.title).unwrap_or_default()
                        }));
                        let open = clone!(on_pick_event event -> move || {
                            if let Some(event) = event.get_untracked() {
                                on_pick_event.call(event);
                            }
                        });
                        view! {
                            <EventChip
                                title={title}
                                @test_id={format!("calendar.event.{id}")}
                                on_click={open}
                            />
                        }
                    }}
                </ForEach>
                <Caption content={overflow} />
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
    let theme = use_theme();
    let click = on_click.clone();
    view! {
        <Frame color={theme.accent.clone()} radius=3>
            <ListRow on_click={move || click.call()} on_activate={move || on_click.call()}>
                <Caption content={title} color={theme.on_accent.clone()} />
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
