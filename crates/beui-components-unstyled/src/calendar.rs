use std::cell::Cell;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate as unstyled;
use crate::ButtonHandle;
use crate::datetime::{Date, Weekday};
use beui_core::base::Direction;
use beui_core::document::Document;
use beui_core::input::{Key, KeyPress};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const WEEKS: usize = 6;
const DAYS_PER_WEEK: usize = 7;
const MONTH_COLUMNS: usize = 3;
const MONTH_ROWS: usize = 4;
const YEAR_COLUMNS: usize = 4;
const YEAR_ROWS: usize = 5;
const YEARS_PER_PAGE: i32 = (YEAR_COLUMNS * YEAR_ROWS) as i32;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CalendarMode {
    #[default]
    Days,
    Months,
    Years,
}

pub struct CalendarHeaderHandle {
    pub month: Memo<Date>,
    pub label: Memo<String>,
    pub month_label: Memo<String>,
    pub year_label: Memo<String>,
    pub mode: ReadSignal<CalendarMode>,
    pub previous: Callback<()>,
    pub next: Callback<()>,
    pub show_months: Callback<()>,
    pub show_years: Callback<()>,
    pub can_previous: Memo<bool>,
    pub can_next: Memo<bool>,
    pub previous_label: Memo<String>,
    pub next_label: Memo<String>,
    pub months_open: Memo<bool>,
    pub years_open: Memo<bool>,
    pub month_toggle: Memo<Node>,
    pub year_toggle: Memo<Node>,
}

pub struct CalendarCellHandle {
    pub kind: CalendarMode,
    pub label: Memo<String>,
    pub selected: Memo<bool>,
    pub marked: Memo<bool>,
    pub outside: Memo<bool>,
    pub disabled: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

#[derive(Clone, Copy)]
struct State {
    selected: ReadSignal<Option<Date>>,
    set_selected: WriteSignal<Option<Date>>,
    active: ReadSignal<Date>,
    set_active: WriteSignal<Date>,
    day_focus: ReadSignal<Option<Date>>,
    set_day_focus: WriteSignal<Option<Date>>,
    month_focus: ReadSignal<Option<Date>>,
    set_month_focus: WriteSignal<Option<Date>>,
    year_focus: ReadSignal<Option<i32>>,
    set_year_focus: WriteSignal<Option<i32>>,
    mode: ReadSignal<CalendarMode>,
    set_mode: WriteSignal<CalendarMode>,
    min: Memo<Option<Date>>,
    max: Memo<Option<Date>>,
    today: Date,
    first_weekday: Weekday,
    on_change: Callback<Date>,
}

#[component]
pub fn Calendar(
    selected: Prop<Option<Date>>,
    #[prop(default = None)] min: Prop<Option<Date>>,
    #[prop(default = None)] max: Prop<Option<Date>>,
    #[prop(default = None)] today: Prop<Option<Date>>,
    #[prop(default = Weekday::Monday)] first_weekday: Weekday,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = None)] show: Prop<Option<Date>>,
    #[prop(default = 0.0)] spacing: f32,
    header: Render<CalendarHeaderHandle>,
    weekday: RenderFn<Weekday>,
    cell: RenderFn<CalendarCellHandle>,
    on_change: Callback<Date>,
) -> NodeId {
    let (day, month, year) = (cell, cell, cell);
    let today = today.peek().unwrap_or_else(Date::today);
    let min = create_memo(move || min.get());
    let max = create_memo(move || max.get());
    let initial = selected.peek();
    let (selected_read, set_selected) = create_signal(initial);
    let starting = initial
        .unwrap_or(today)
        .clamp_to(min.get_untracked(), max.get_untracked());
    let (active, set_active) = create_signal(starting);
    let (day_focus, set_day_focus) = create_signal(None);
    let (month_focus, set_month_focus) = create_signal(None);
    let (year_focus, set_year_focus) = create_signal(None);
    let (mode, set_mode) = create_signal(CalendarMode::Days);
    let state = State {
        selected: selected_read,
        set_selected,
        active,
        set_active,
        day_focus,
        set_day_focus,
        month_focus,
        set_month_focus,
        year_focus,
        set_year_focus,
        mode,
        set_mode,
        min,
        max,
        today,
        first_weekday,
        on_change,
    };
    set_component_state(state);

    create_effect(move || {
        let requested = selected.get();
        state.set_selected.set(requested);
        if let Some(date) = requested {
            state.set_active.set(clamp(state, date));
        }
    });
    create_effect(move || {
        if let Some(date) = show.get() {
            state.set_active.set(clamp(state, date));
            state.set_mode.set(CalendarMode::Days);
        }
    });
    create_effect(move || {
        if focused.get() {
            reset(state);
            state.set_mode.set(CalendarMode::Days);
            state.set_day_focus.set(Some(state.active.get_untracked()));
        }
    });

    let shown_month = create_memo(move || active.get().first_of_month());
    let first_year = create_memo(move || page_start(active.get().year));
    let label = create_memo(move || match mode.get() {
        CalendarMode::Days => shown_month.get().month_label(),
        CalendarMode::Months => shown_month.get().year.to_string(),
        CalendarMode::Years => years_label(first_year.get()),
    });
    let month_label = create_memo(move || shown_month.get().month_name().to_owned());
    let year_label = create_memo(move || match mode.get() {
        CalendarMode::Years => years_label(first_year.get()),
        CalendarMode::Days | CalendarMode::Months => shown_month.get().year.to_string(),
    });
    let can_previous = create_memo(move || {
        let edge = match state.mode.get() {
            CalendarMode::Days => shown_month.get().add_days(-1),
            CalendarMode::Months => Date::new(shown_month.get().year - 1, 12, 31),
            CalendarMode::Years => Date::new(first_year.get() - 1, 12, 31),
        };
        edge.year >= Date::MIN_YEAR && state.min.get().is_none_or(|min| edge >= min)
    });
    let can_next = create_memo(move || {
        let edge = match state.mode.get() {
            CalendarMode::Days => shown_month.get().last_of_month().add_days(1),
            CalendarMode::Months => Date::new(shown_month.get().year + 1, 1, 1),
            CalendarMode::Years => Date::new(first_year.get() + YEARS_PER_PAGE, 1, 1),
        };
        edge.year <= Date::MAX_YEAR && state.max.get().is_none_or(|max| edge <= max)
    });
    let unit = create_memo(move || match mode.get() {
        CalendarMode::Days => "month",
        CalendarMode::Months => "year",
        CalendarMode::Years => "years",
    });
    let previous_label = create_memo(move || format!("Previous {}", unit.get()));
    let next_label = create_memo(move || format!("Next {}", unit.get()));
    let months_open = create_memo(move || mode.get() == CalendarMode::Months);
    let years_open = create_memo(move || mode.get() == CalendarMode::Years);
    let month_toggle = toggle_accessibility(month_label, "choose a month", months_open);
    let year_toggle = toggle_accessibility(year_label, "choose a year", years_open);
    let header_node = header.call(CalendarHeaderHandle {
        month: shown_month,
        label,
        month_label,
        year_label,
        mode,
        previous: Callback::new(move |()| page(state, -1)),
        next: Callback::new(move |()| page(state, 1)),
        show_months: Callback::new(move |()| show_mode(state, CalendarMode::Months)),
        show_years: Callback::new(move |()| show_mode(state, CalendarMode::Years)),
        can_previous,
        can_next,
        previous_label,
        next_label,
        months_open,
        years_open,
        month_toggle,
        year_toggle,
    });

    let grid_start =
        create_memo(move || active.get().first_of_month().start_of_week(first_weekday));
    let days_shown = create_memo(move || mode.get() == CalendarMode::Days);
    let months_shown = create_memo(move || mode.get() == CalendarMode::Months);
    let years_shown = create_memo(move || mode.get() == CalendarMode::Years);
    let days_label = label;
    let months_label = label;
    view! {
        <List spacing>
            {header_node}
            <Show condition={days_shown}>|| {
                let (days_label, spacing, weekday) = (days_label, spacing, weekday);
                let (grid_start, day) = (grid_start, day);
                view! {
                    <CalendarGrid label=days_label spacing>
                        <List direction=Direction::Horizontal spacing>
                            <ForEach keys={(0..DAYS_PER_WEEK).collect::<Vec<_>>()}>|column: usize|
                                <CalendarWeekday
                                    @sizing=ItemSize::Percent(100.0)
                                    weekday={first_weekday.offset(column as u32)}
                                    face={weekday}
                                />
                            </ForEach>
                        </List>
                        <ForEach keys={(0..WEEKS).collect::<Vec<_>>()}>|week: usize|
                            <CalendarWeek
                                state={state}
                                week
                                grid_start={grid_start}
                                spacing
                                day={day}
                            />
                        </ForEach>
                    </CalendarGrid>
                }
            }</Show>
            <Show condition={months_shown}>|| {
                let (months_label, spacing) = (months_label, spacing);
                view! {
                    <CalendarGrid label=months_label spacing>
                        <ForEach keys={(0..MONTH_ROWS).collect::<Vec<_>>()}>|row: usize|
                            <CalendarMonthRow state={state} row spacing month={month} />
                        </ForEach>
                    </CalendarGrid>
                }
            }</Show>
            <Show condition={years_shown}>|| {
                let (label, spacing) = (label, spacing);
                let (first_year, year) = (first_year, year);
                view! {
                    <CalendarGrid label spacing>
                        <ForEach keys={(0..YEAR_ROWS).collect::<Vec<_>>()}>|row: usize|
                            <CalendarYearRow
                                state={state}
                                row
                                first_year={first_year}
                                spacing
                                year={year}
                            />
                        </ForEach>
                    </CalendarGrid>
                }
            }</Show>
        </List>
    }
}

#[component]
fn CalendarYearRow(
    state: State,
    row: usize,
    first_year: Memo<i32>,
    spacing: f32,
    year: RenderFn<CalendarCellHandle>,
) -> NodeId {
    component_accessibility(Node::new(Role::Row));
    view! {
        <List direction=Direction::Horizontal spacing>
            <ForEach keys={(0..YEAR_COLUMNS).collect::<Vec<_>>()}>|column: usize|
                <CalendarYear
                    @sizing=ItemSize::Percent(100.0)
                    state={state}
                    offset={(row * YEAR_COLUMNS + column) as i32}
                    first_year={first_year}
                    face={year}
                />
            </ForEach>
        </List>
    }
}

#[component]
fn CalendarYear(
    state: State,
    offset: i32,
    first_year: Memo<i32>,
    face: RenderFn<CalendarCellHandle>,
) -> NodeId {
    let year = create_memo(move || first_year.get() + offset);
    let selected = create_memo(move || {
        state
            .selected
            .get()
            .is_some_and(|selected| selected.year == year.get())
    });
    let current = create_memo(move || state.today.year == year.get());
    let disabled = create_memo(move || {
        let year = year.get();
        !(Date::MIN_YEAR..=Date::MAX_YEAR).contains(&year)
            || state.min.get().is_some_and(|min| year < min.year)
            || state.max.get().is_some_and(|max| year > max.year)
    });
    let tab_stop = create_memo(move || state.active.get().year == year.get());
    let focused = create_memo(move || state.year_focus.get() == Some(year.get()));
    let accessibility = create_memo(move || {
        let mut node = Node::new(Role::GridCell);
        node.set_label(year.get().to_string());
        node.set_selected(selected.get());
        node
    });
    let (focus_year, click_year, key_year) = (year, year, year);
    let focused_as = Cell::new(None);
    let handle_disabled = disabled;
    view! {
        <unstyled::Button
            tab_stop
            focused
            disabled
            accessibility
            on_focus_change={move |has_focus: bool| {
                if has_focus {
                    let year = focus_year.get_untracked();
                    focused_as.set(Some(year));
                    state.set_year_focus.set(Some(year));
                } else if let Some(year) = focused_as.take()
                    && state.year_focus.get_untracked() == Some(year)
                {
                    state.set_year_focus.set(None);
                }
            }}
            on_click={move || pick_year(state, click_year.get_untracked())}
            on_key={move |press: KeyPress| year_key(state, key_year.get_untracked(), press)}
        >|button: ButtonHandle| {
            face.call(CalendarCellHandle {
                kind: CalendarMode::Years,
                label: create_memo(move || year.get().to_string()),
                selected,
                marked: current,
                outside: create_memo(|| false),
                disabled: handle_disabled,
                hovered: button.hovered,
                active: button.active,
                focused: button.focused,
            })
        }</unstyled::Button>
    }
}

#[component]
fn CalendarGrid(
    label: Memo<String>,
    spacing: f32,
    children: beui_view::reactive::Children<beui_view::reactive::ListChild>,
) -> NodeId {
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Grid);
        node.set_label(label.get());
        node
    }));
    view! {
        <List spacing children />
    }
}

#[component]
fn CalendarWeekday(weekday: Weekday, face: RenderFn<Weekday>) -> NodeId {
    let mut node = Node::new(Role::ColumnHeader);
    node.set_label(weekday.name());
    component_accessibility(node);
    view! {
        <List spacing=0.0>{face.call(weekday)}</List>
    }
}

#[component]
fn CalendarWeek(
    state: State,
    week: usize,
    grid_start: Memo<Date>,
    spacing: f32,
    day: RenderFn<CalendarCellHandle>,
) -> NodeId {
    component_accessibility(Node::new(Role::Row));
    view! {
        <List direction=Direction::Horizontal spacing>
            <ForEach keys={(0..DAYS_PER_WEEK).collect::<Vec<_>>()}>|column: usize|
                <CalendarDay
                    @sizing=ItemSize::Percent(100.0)
                    state={state}
                    offset={(week * DAYS_PER_WEEK + column) as i64}
                    grid_start={grid_start}
                    face={day}
                />
            </ForEach>
        </List>
    }
}

#[component]
fn CalendarDay(
    state: State,
    offset: i64,
    grid_start: Memo<Date>,
    face: RenderFn<CalendarCellHandle>,
) -> NodeId {
    let date = create_memo(move || grid_start.get().add_days(offset));
    let selected = create_memo(move || state.selected.get() == Some(date.get()));
    let today = create_memo(move || date.get() == state.today);
    let outside = create_memo(move || !date.get().same_month(state.active.get()));
    let disabled = create_memo(move || !in_range(state, date.get()));
    let tab_stop = create_memo(move || state.active.get() == date.get());
    let focused = create_memo(move || state.day_focus.get() == Some(date.get()));
    let accessibility = create_memo(move || {
        let date = date.get();
        let mut node = Node::new(Role::GridCell);
        let mut label = date.label();
        if today.get() {
            label.push_str(", today");
        }
        node.set_label(label);
        node.set_selected(selected.get());
        node
    });
    let (focus_date, click_date, key_date) = (date, date, date);
    let focused_as = Cell::new(None);
    let handle_disabled = disabled;
    view! {
        <unstyled::Button
            tab_stop
            focused
            disabled
            accessibility
            on_focus_change={move |has_focus: bool| {
                if has_focus {
                    let date = focus_date.get_untracked();
                    focused_as.set(Some(date));
                    state.set_day_focus.set(Some(date));
                    state.set_active.set(date);
                } else if let Some(date) = focused_as.take()
                    && state.day_focus.get_untracked() == Some(date)
                {
                    state.set_day_focus.set(None);
                }
            }}
            on_click={move || choose(state, click_date.get_untracked())}
            on_key={move |press: KeyPress| day_key(state, key_date.get_untracked(), press)}
        >|button: ButtonHandle| {
            face.call(CalendarCellHandle {
                kind: CalendarMode::Days,
                label: create_memo(move || date.get().day.to_string()),
                selected,
                marked: today,
                outside,
                disabled: handle_disabled,
                hovered: button.hovered,
                active: button.active,
                focused: button.focused,
            })
        }</unstyled::Button>
    }
}

#[component]
fn CalendarMonthRow(
    state: State,
    row: usize,
    spacing: f32,
    month: RenderFn<CalendarCellHandle>,
) -> NodeId {
    component_accessibility(Node::new(Role::Row));
    view! {
        <List direction=Direction::Horizontal spacing>
            <ForEach keys={(0..MONTH_COLUMNS).collect::<Vec<_>>()}>|column: usize|
                <CalendarMonth
                    @sizing=ItemSize::Percent(100.0)
                    state={state}
                    number={(row * MONTH_COLUMNS + column + 1) as u8}
                    face={month}
                />
            </ForEach>
        </List>
    }
}

#[component]
fn CalendarMonth(state: State, number: u8, face: RenderFn<CalendarCellHandle>) -> NodeId {
    let month = create_memo(move || Date::new(state.active.get().year, number, 1));
    let selected = create_memo(move || {
        state
            .selected
            .get()
            .is_some_and(|selected| selected.same_month(month.get()))
    });
    let current = create_memo(move || state.today.same_month(month.get()));
    let disabled = create_memo(move || {
        let month = month.get();
        state
            .min
            .get()
            .is_some_and(|min| month.last_of_month() < min)
            || state.max.get().is_some_and(|max| month > max)
    });
    let tab_stop = create_memo(move || state.active.get().month == number);
    let focused = create_memo(move || state.month_focus.get() == Some(month.get()));
    let accessibility = create_memo(move || {
        let mut node = Node::new(Role::GridCell);
        node.set_label(month.get().month_label());
        node.set_selected(selected.get());
        node
    });
    let (focus_month, click_month) = (month, month);
    let focused_as = Cell::new(None);
    let handle_disabled = disabled;
    view! {
        <unstyled::Button
            tab_stop
            focused
            disabled
            accessibility
            on_focus_change={move |has_focus: bool| {
                if has_focus {
                    let month = focus_month.get_untracked();
                    focused_as.set(Some(month));
                    state.set_month_focus.set(Some(month));
                } else if let Some(month) = focused_as.take()
                    && state.month_focus.get_untracked() == Some(month)
                {
                    state.set_month_focus.set(None);
                }
            }}
            on_click={move || pick_month(state, click_month.get_untracked())}
            on_key={move |press: KeyPress| month_key(state, number, press)}
        >|button: ButtonHandle| {
            face.call(CalendarCellHandle {
                kind: CalendarMode::Months,
                label: create_memo(move || month.get().month_name()[..3].to_owned()),
                selected,
                marked: current,
                outside: create_memo(|| false),
                disabled: handle_disabled,
                hovered: button.hovered,
                active: button.active,
                focused: button.focused,
            })
        }</unstyled::Button>
    }
}

fn toggle_accessibility(label: Memo<String>, choose: &'static str, open: Memo<bool>) -> Memo<Node> {
    create_memo(move || {
        let mut node = Node::new(Role::Button);
        node.set_label(match open.get() {
            true => format!("{}, back to the days", label.get()),
            false => format!("{}, {choose}", label.get()),
        });
        node.set_expanded(open.get());
        node
    })
}

pub fn calendar_selected(document: &Document, calendar: NodeId) -> Option<Date> {
    document
        .component_state::<State>(calendar)
        .selected
        .get_untracked()
}

pub fn calendar_active(document: &Document, calendar: NodeId) -> Date {
    document
        .component_state::<State>(calendar)
        .active
        .get_untracked()
}

pub fn calendar_mode(document: &Document, calendar: NodeId) -> CalendarMode {
    document
        .component_state::<State>(calendar)
        .mode
        .get_untracked()
}

fn in_range(state: State, date: Date) -> bool {
    state.min.get().is_none_or(|min| date >= min) && state.max.get().is_none_or(|max| date <= max)
}

fn clamp(state: State, date: Date) -> Date {
    date.clamp_to(state.min.get_untracked(), state.max.get_untracked())
}

fn reset(state: State) {
    let date = state.selected.get_untracked().unwrap_or(state.today);
    state.set_active.set(clamp(state, date));
}

fn choose(state: State, date: Date) {
    let date = clamp(state, date);
    state.set_selected.set(Some(date));
    state.set_active.set(date);
    state.on_change.call(date);
}

fn page(state: State, direction: i32) {
    let active = state.active.get_untracked();
    let next = match state.mode.get_untracked() {
        CalendarMode::Days => active.add_months(direction),
        CalendarMode::Months => active.add_years(direction),
        CalendarMode::Years => active.add_years(direction * YEARS_PER_PAGE),
    };
    state.set_active.set(clamp(state, next));
}

fn show_mode(state: State, wanted: CalendarMode) {
    state.set_mode.update(|mode| {
        *mode = match *mode == wanted {
            true => CalendarMode::Days,
            false => wanted,
        }
    });
}

fn page_start(year: i32) -> i32 {
    year - year.rem_euclid(YEARS_PER_PAGE)
}

fn years_label(first: i32) -> String {
    format!(
        "{}–{}",
        first.max(Date::MIN_YEAR),
        (first + YEARS_PER_PAGE - 1).min(Date::MAX_YEAR)
    )
}

fn pick_year(state: State, year: i32) {
    let active = state.active.get_untracked();
    let date = clamp(state, Date::new(year, active.month, active.day));
    state.set_active.set(date);
    state.set_mode.set(CalendarMode::Months);
    state.set_year_focus.set(None);
    state.set_month_focus.set(Some(date.first_of_month()));
}

fn year_key(state: State, year: i32, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let first = page_start(year);
    let columns = YEAR_COLUMNS as i32;
    let next = match press.key {
        Key::ArrowLeft => year - 1,
        Key::ArrowRight => year + 1,
        Key::ArrowUp => year - columns,
        Key::ArrowDown => year + columns,
        Key::Home => first,
        Key::End => first + YEARS_PER_PAGE - 1,
        Key::PageUp => year - YEARS_PER_PAGE,
        Key::PageDown => year + YEARS_PER_PAGE,
        _ => return false,
    };
    if press.pressed {
        let active = state.active.get_untracked();
        let next = clamp(state, Date::new(next, active.month, active.day));
        state.set_active.set(next);
        state.set_year_focus.set(Some(next.year));
    }
    true
}

fn pick_month(state: State, month: Date) {
    let active = state.active.get_untracked();
    let date = clamp(state, Date::new(month.year, month.month, active.day));
    state.set_active.set(date);
    state.set_mode.set(CalendarMode::Days);
    state.set_month_focus.set(None);
    state.set_day_focus.set(Some(date));
}

fn day_key(state: State, date: Date, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let next = match press.key {
        Key::ArrowLeft => date.add_days(-1),
        Key::ArrowRight => date.add_days(1),
        Key::ArrowUp => date.add_days(-(DAYS_PER_WEEK as i64)),
        Key::ArrowDown => date.add_days(DAYS_PER_WEEK as i64),
        Key::Home => date.start_of_week(state.first_weekday),
        Key::End => date
            .start_of_week(state.first_weekday)
            .add_days(DAYS_PER_WEEK as i64 - 1),
        Key::PageUp if press.modifiers.shift => date.add_years(-1),
        Key::PageDown if press.modifiers.shift => date.add_years(1),
        Key::PageUp => date.add_months(-1),
        Key::PageDown => date.add_months(1),
        _ => return false,
    };
    if press.pressed {
        let next = clamp(state, next);
        state.set_active.set(next);
        state.set_day_focus.set(Some(next));
    }
    true
}

fn month_key(state: State, number: u8, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let columns = MONTH_COLUMNS as i32;
    let offset = match press.key {
        Key::ArrowLeft => -1,
        Key::ArrowRight => 1,
        Key::ArrowUp => -columns,
        Key::ArrowDown => columns,
        Key::Home => 1 - i32::from(number),
        Key::End => 12 - i32::from(number),
        Key::PageUp => -12,
        Key::PageDown => 12,
        _ => return false,
    };
    if press.pressed {
        let active = state.active.get_untracked();
        let next = clamp(
            state,
            Date::new(active.year, number, active.day).add_months(offset),
        );
        state.set_active.set(next);
        state.set_month_focus.set(Some(next.first_of_month()));
    }
    true
}
