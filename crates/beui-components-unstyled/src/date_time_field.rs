use std::cell::Cell;
use std::time::Duration;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::datetime::{Date, DateTime, HourCycle, MONTH_NAMES, Time, days_in_month, twelve_hour};
use beui_view::types::Direction;
use beui_view::types::{CursorIcon, Key, KeyPress};
use beui_view::reactive::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const LEAP_YEAR: i32 = 2000;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum DateTimeParts {
    Date,
    Time,
    #[default]
    DateTime,
}

impl DateTimeParts {
    pub fn has_date(self) -> bool {
        self != DateTimeParts::Time
    }

    pub fn has_time(self) -> bool {
        self != DateTimeParts::Date
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum DateSegment {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Period,
}

impl DateSegment {
    fn index(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            DateSegment::Year => "Year",
            DateSegment::Month => "Month",
            DateSegment::Day => "Day",
            DateSegment::Hour => "Hour",
            DateSegment::Minute => "Minute",
            DateSegment::Period => "AM/PM",
        }
    }

    pub fn placeholder(self) -> &'static str {
        match self {
            DateSegment::Year => "yyyy",
            DateSegment::Month => "mm",
            DateSegment::Day => "dd",
            DateSegment::Hour => "hh",
            DateSegment::Minute => "mm",
            DateSegment::Period => "am",
        }
    }

    fn digits(self) -> usize {
        match self {
            DateSegment::Year => 4,
            DateSegment::Period => 0,
            _ => 2,
        }
    }

    fn page(self, cycle: HourCycle) -> i32 {
        match (self, cycle) {
            (DateSegment::Year, _) => 10,
            (DateSegment::Month, _) => 3,
            (DateSegment::Day, _) => 7,
            (DateSegment::Hour, HourCycle::H24) => 6,
            (DateSegment::Hour, HourCycle::H12) => 3,
            (DateSegment::Minute, _) => 15,
            (DateSegment::Period, _) => 1,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct DateDraft {
    pub year: Option<i32>,
    pub month: Option<u8>,
    pub day: Option<u8>,
}

pub struct DateSegmentHandle {
    pub segment: DateSegment,
    pub text: Memo<String>,
    pub placeholder: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
}

type Values = [Option<i32>; 6];

#[derive(Clone, Copy)]
struct State {
    parts: DateTimeParts,
    cycle: HourCycle,
    order: Stored<Vec<DateSegment>>,
    values: Signal<Values>,
    typing: Signal<Option<(DateSegment, String)>>,
    focus: Signal<Option<DateSegment>>,
    value: Binding<Option<DateTime>>,
    disabled: Memo<bool>,
    left: Timer,
    on_open: Callback<DateSegment>,
}

#[component]
pub fn DateTimeField(
    #[prop(default = ComponentRef::new())] api: ComponentRef<DateTimeFieldApi>,
    #[prop(default = None)] value: Binding<Option<DateTime>>,
    #[prop(default = DateTimeParts::DateTime)] parts: DateTimeParts,
    #[prop(default = HourCycle::H24)] hour_cycle: HourCycle,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = None)] focus_segment: Prop<Option<DateSegment>>,
    #[prop(default = String::new())] label: Prop<String>,
    segment: RenderFn<DateSegmentHandle>,
    literal: RenderFn<String>,
    on_focus_change: Callback<bool>,
    on_segment_focus: Callback<Option<DateSegment>>,
    on_draft: Callback<DateDraft>,
    on_open: Callback<DateSegment>,
) -> NodeId {
    let order = segments(parts, hour_cycle);
    let count = order.len();
    let first = order[0];
    let initial = value.get_untracked();
    let values = create_signal(split(initial, hour_cycle));
    let typing = create_signal(None);
    let focus = create_signal(None);
    let disabled = create_memo(move || disabled.get());
    let left_state = store(Cell::new(None::<State>));
    let left = create_timer(move || {
        if let Some(state) = left_state.try_with(Cell::get).flatten() {
            left_field(state);
        }
        None
    });
    let state = State {
        parts,
        cycle: hour_cycle,
        order: store(order),
        values,
        typing,
        focus,
        value,
        disabled,
        left,
        on_open,
    };
    left_state.with(|left| left.set(Some(state)));
    api.fill(DateTimeFieldApi(state));

    create_effect(move || {
        let next = value.get();
        let values = state.values.get_untracked();
        if compose(state, &values, next) != next || (next.is_none() && !empty(state, &values)) {
            state.values.set(split(next, state.cycle));
            state.typing.set(None);
        }
    });
    create_effect(move || {
        if disabled.get() {
            state.focus.set(None);
        }
    });
    create_effect(move || {
        if let Some(wanted) = focus_segment.get() {
            let segment = match state.order.value().contains(&wanted) {
                true => wanted,
                false => state.order.value()[0],
            };
            state.focus.set(Some(segment));
        }
    });
    create_effect(move || on_segment_focus.call(state.focus.get()));
    create_effect(move || {
        let values = state.values.get();
        let typed = state.typing.get();
        let read = |segment: DateSegment| match &typed {
            Some((typing, buffer)) if *typing == segment && segment != DateSegment::Year => {
                buffer.parse::<i32>().ok().filter(|number| *number > 0)
            }
            _ => values[segment.index()],
        };
        on_draft.call(DateDraft {
            year: read(DateSegment::Year),
            month: read(DateSegment::Month).map(|month| month.clamp(1, 12) as u8),
            day: read(DateSegment::Day).map(|day| day.clamp(1, 31) as u8),
        });
    });
    let focus = state.focus;
    let within = create_memo(move || focus.get().is_some());
    create_effect(move || on_focus_change.call(within.get()));
    create_effect(move || {
        if focused.get() {
            state.focus.set(Some(first));
        }
    });

    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Group);
        let label = label.get();
        if !label.is_empty() {
            node.set_label(label);
        }
        node
    }));

    let items: Vec<usize> = (0..count).collect();
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <ForEach keys={items}>|index: usize| {
                let order = state.order.value();
                let segment_kind = order[index];
                let prefix = match index {
                    0 => String::new(),
                    _ => separator(order[index - 1], segment_kind),
                };
                view! {
                    <SegmentView state segment=segment_kind prefix literal face={segment} />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn SegmentView(
    state: State,
    segment: DateSegment,
    prefix: String,
    literal: RenderFn<String>,
    face: RenderFn<DateSegmentHandle>,
) -> NodeId {
    let hovered = create_signal(false);
    let is_focused = create_signal(false);
    let focused = create_memo(move || state.focus.get() == Some(segment));
    let tab_stop = create_memo(move || !state.disabled.get());
    let value = create_memo(move || state.values.get()[segment.index()]);
    let typed = create_memo(move || match state.typing.get() {
        Some((typing, buffer)) if typing == segment => Some(buffer),
        _ => None,
    });
    let text = create_memo(move || match typed.get() {
        Some(buffer) => buffer,
        None => value.get().map_or_else(
            || segment.placeholder().to_owned(),
            |value| format_segment(segment, value),
        ),
    });
    let placeholder = create_memo(move || typed.get().is_none() && value.get().is_none());
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::SpinButton);
        node.set_label(segment.label());
        let (min, max) = range(state, segment, &state.values.get());
        node.set_min_numeric_value(f64::from(min));
        node.set_max_numeric_value(f64::from(max));
        node.set_numeric_value_step(1.0);
        match value.get() {
            Some(value) => {
                node.set_numeric_value(f64::from(value));
                node.set_value(spoken(segment, value));
            }
            None => node.set_value("empty"),
        }
        if state.disabled.get() {
            node.set_disabled();
        }
        node
    }));
    let content = face.call(DateSegmentHandle {
        segment,
        text,
        placeholder,
        hovered: hovered.read_only(),
        focused: is_focused.read_only(),
        disabled: state.disabled,
    });
    let shown = !prefix.is_empty();
    view! {
        <Interactive
            focusable=true
            tab_stop
            focused
            ime={is_focused}
            on_focus_change={move |has_focus: bool| {
                is_focused.set(has_focus);
                segment_focus(state, segment, has_focus);
            }}
            on_step={move |delta: f32| step(state, segment, delta.round() as i32)}
            on_key={move |press: KeyPress| segment_key(state, segment, press)}
            on_text={move |typed: String| type_text(state, segment, &typed)}
            cursor=CursorIcon::Text
            on_hover_change={hovered}
        >
            <List direction=Direction::Horizontal spacing=0.0>
                <Show condition=shown>|| { literal.call(prefix.clone()) }</Show>
                {content}
            </List>
        </Interactive>
    }
}

fn segments(parts: DateTimeParts, cycle: HourCycle) -> Vec<DateSegment> {
    let mut order = Vec::new();
    if parts.has_date() {
        order.extend([DateSegment::Year, DateSegment::Month, DateSegment::Day]);
    }
    if parts.has_time() {
        order.extend([DateSegment::Hour, DateSegment::Minute]);
        if cycle == HourCycle::H12 {
            order.push(DateSegment::Period);
        }
    }
    order
}

fn separator(before: DateSegment, after: DateSegment) -> String {
    match (before, after) {
        (DateSegment::Day, DateSegment::Hour) | (DateSegment::Minute, DateSegment::Period) => {
            " ".to_owned()
        }
        (DateSegment::Hour, DateSegment::Minute) => ":".to_owned(),
        _ => "-".to_owned(),
    }
}

fn format_segment(segment: DateSegment, value: i32) -> String {
    match segment {
        DateSegment::Year => format!("{value:04}"),
        DateSegment::Period => match value {
            0 => "AM".to_owned(),
            _ => "PM".to_owned(),
        },
        _ => format!("{value:02}"),
    }
}

fn spoken(segment: DateSegment, value: i32) -> String {
    match segment {
        DateSegment::Month => MONTH_NAMES
            .get((value - 1).clamp(0, 11) as usize)
            .copied()
            .unwrap_or_default()
            .to_owned(),
        DateSegment::Period => format_segment(segment, value),
        _ => value.to_string(),
    }
}

fn split(value: Option<DateTime>, cycle: HourCycle) -> Values {
    let Some(value) = value else {
        return [None; 6];
    };
    let hour = match cycle {
        HourCycle::H24 => value.time.hour,
        HourCycle::H12 => twelve_hour(value.time.hour),
    };
    [
        Some(value.date.year),
        Some(i32::from(value.date.month)),
        Some(i32::from(value.date.day)),
        Some(i32::from(hour)),
        Some(i32::from(value.time.minute)),
        Some(i32::from(value.time.hour >= 12)),
    ]
}

fn range(state: State, segment: DateSegment, values: &Values) -> (i32, i32) {
    match segment {
        DateSegment::Year => (Date::MIN_YEAR, Date::MAX_YEAR),
        DateSegment::Month => (1, 12),
        DateSegment::Day => {
            let year = values[DateSegment::Year.index()].unwrap_or(LEAP_YEAR);
            let days = values[DateSegment::Month.index()]
                .map_or(31, |month| days_in_month(year, month.clamp(1, 12) as u8));
            (1, i32::from(days))
        }
        DateSegment::Hour => match state.cycle {
            HourCycle::H24 => (0, 23),
            HourCycle::H12 => (1, 12),
        },
        DateSegment::Minute => (0, 59),
        DateSegment::Period => (0, 1),
    }
}

fn compose(state: State, values: &Values, base: Option<DateTime>) -> Option<DateTime> {
    let get = |segment: DateSegment| values[segment.index()];
    let date = match state.parts.has_date() {
        true => Date::new(
            get(DateSegment::Year)?,
            get(DateSegment::Month)? as u8,
            get(DateSegment::Day)? as u8,
        ),
        false => base.map_or(Date::EPOCH, |base| base.date),
    };
    let time = match state.parts.has_time() {
        true => {
            let hour = get(DateSegment::Hour)? as u8;
            let hour = match state.cycle {
                HourCycle::H24 => hour,
                HourCycle::H12 => {
                    hour % 12
                        + if get(DateSegment::Period)? == 1 {
                            12
                        } else {
                            0
                        }
                }
            };
            Time::new(hour, get(DateSegment::Minute)? as u8)
        }
        false => base.map_or(Time::MIDNIGHT, |base| base.time),
    };
    Some(DateTime::new(date, time))
}

fn empty(state: State, values: &Values) -> bool {
    state.order.with(|order| {
        order
            .iter()
            .all(|segment| values[segment.index()].is_none())
    })
}

fn commit(state: State, mut values: Values) {
    let current = state.value.get_untracked();
    let composed = compose(state, &values, current);
    if let Some(composed) = composed
        && state.parts.has_date()
    {
        values[DateSegment::Day.index()] = Some(i32::from(composed.date.day));
    }
    state.values.set(values);
    let reported = match composed {
        Some(composed) => Some(composed),
        None if empty(state, &values) => None,
        None => return,
    };
    if reported != current {
        state.value.set(reported);
    }
}

fn now_value(state: State, segment: DateSegment) -> i32 {
    let now = DateTime::now();
    split(Some(now), state.cycle)[segment.index()].unwrap_or(0)
}

fn step(state: State, segment: DateSegment, delta: i32) {
    if state.disabled.get_untracked() || delta == 0 {
        return;
    }
    finish_typing(state);
    let mut values = state.values.get_untracked();
    let (min, max) = range(state, segment, &values);
    let next = match values[segment.index()] {
        None => now_value(state, segment).clamp(min, max),
        Some(value) => min + (value - min + delta).rem_euclid(max - min + 1),
    };
    values[segment.index()] = Some(next);
    commit(state, values);
}

fn set_to(state: State, segment: DateSegment, value: Option<i32>) {
    let mut values = state.values.get_untracked();
    values[segment.index()] = value;
    state.typing.set(None);
    commit(state, values);
}

fn finish_typing(state: State) {
    let Some((segment, buffer)) = state.typing.get_untracked() else {
        return;
    };
    state.typing.set(None);
    let Ok(number) = buffer.parse::<i32>() else {
        return;
    };
    if segment == DateSegment::Year && buffer.len() < segment.digits() {
        return;
    }
    let values = state.values.get_untracked();
    let (min, max) = range(state, segment, &values);
    set_to(state, segment, Some(number.clamp(min, max)));
}

fn neighbour(state: State, segment: DateSegment, offset: isize) -> Option<DateSegment> {
    let index = state
        .order
        .value()
        .iter()
        .position(|each| *each == segment)? as isize
        + offset;
    usize::try_from(index)
        .ok()
        .and_then(|index| state.order.value().get(index).copied())
}

fn advance(state: State, segment: DateSegment) -> DateSegment {
    match neighbour(state, segment, 1) {
        Some(next) => {
            state.focus.set(Some(next));
            next
        }
        None => segment,
    }
}

fn segment_focus(state: State, segment: DateSegment, has_focus: bool) {
    if has_focus {
        state.left.stop();
        state.focus.set(Some(segment));
        return;
    }
    if state
        .typing
        .get_untracked()
        .is_some_and(|(typing, _)| typing == segment)
    {
        finish_typing(state);
    }
    if state.focus.get_untracked() == Some(segment) {
        state.focus.set(None);
    }
    state.left.restart(Duration::ZERO);
}

fn left_field(state: State) {
    if state.focus.get_untracked().is_some() {
        return;
    }
    let values = state.values.get_untracked();
    let current = state.value.get_untracked();
    if compose(state, &values, current).is_none() && !empty(state, &values) {
        state.values.set(split(current, state.cycle));
    }
}

fn segment_key(state: State, segment: DateSegment, press: KeyPress) -> bool {
    if press.key == Key::ArrowDown
        && press.modifiers.alt
        && !press.modifiers.ctrl
        && !state.disabled.get_untracked()
    {
        if press.pressed {
            state.on_open.call(segment);
        }
        return true;
    }
    if press.modifiers.ctrl || press.modifiers.alt || state.disabled.get_untracked() {
        return false;
    }
    let page = segment.page(state.cycle);
    match press.key {
        Key::ArrowUp | Key::ArrowDown | Key::PageUp | Key::PageDown => {
            if press.pressed {
                let delta = match press.key {
                    Key::ArrowUp => 1,
                    Key::ArrowDown => -1,
                    Key::PageUp => page,
                    _ => -page,
                };
                step(state, segment, delta);
            }
            true
        }
        Key::Home | Key::End => {
            if press.pressed {
                finish_typing(state);
                let (min, max) = range(state, segment, &state.values.get_untracked());
                set_to(
                    state,
                    segment,
                    Some(if press.key == Key::Home { min } else { max }),
                );
            }
            true
        }
        Key::ArrowLeft | Key::ArrowRight => {
            let offset = if press.key == Key::ArrowLeft { -1 } else { 1 };
            if press.pressed
                && let Some(next) = neighbour(state, segment, offset)
            {
                state.focus.set(Some(next));
            }
            true
        }
        Key::Backspace | Key::Delete => {
            if press.pressed {
                let typing = state
                    .typing
                    .get_untracked()
                    .filter(|(typing, _)| *typing == segment);
                let value = state.values.get_untracked()[segment.index()];
                match (typing, value) {
                    (Some((_, mut buffer)), _)
                        if press.key == Key::Backspace && buffer.len() > 1 =>
                    {
                        buffer.pop();
                        state.typing.set(Some((segment, buffer)));
                    }
                    (None, None) if press.key == Key::Backspace => {
                        if let Some(previous) = neighbour(state, segment, -1) {
                            state.focus.set(Some(previous));
                        }
                    }
                    _ => set_to(state, segment, None),
                }
            }
            true
        }
        _ => false,
    }
}

fn type_text(state: State, segment: DateSegment, text: &str) {
    if state.disabled.get_untracked() {
        return;
    }
    let mut segment = segment;
    for character in text.chars() {
        if segment == DateSegment::Period {
            match character.to_ascii_lowercase() {
                'a' => set_to(state, segment, Some(0)),
                'p' => set_to(state, segment, Some(1)),
                _ => continue,
            }
            segment = advance(state, segment);
            continue;
        }
        if let Some(digit) = character.to_digit(10) {
            segment = type_digit(state, segment, digit as i32, character);
            continue;
        }
        if matches!(character, '-' | '/' | ':' | '.' | ' ' | ',') {
            let typed = state
                .typing
                .get_untracked()
                .is_some_and(|(typing, _)| typing == segment);
            let filled = state.values.get_untracked()[segment.index()].is_some();
            if typed || filled {
                finish_typing(state);
                segment = advance(state, segment);
            }
        }
    }
}

fn type_digit(state: State, segment: DateSegment, digit: i32, character: char) -> DateSegment {
    let mut buffer = match state.typing.get_untracked() {
        Some((typing, buffer)) if typing == segment => buffer,
        _ => String::new(),
    };
    buffer.push(character);
    let values = state.values.get_untracked();
    let (min, max) = range(state, segment, &values);
    let mut number = buffer.parse::<i32>().unwrap_or(digit);
    if number > max {
        buffer = character.to_string();
        number = digit;
    }
    let complete = buffer.len() >= segment.digits() || number * 10 > max;
    if !complete {
        state.typing.set(Some((segment, buffer)));
        return segment;
    }
    set_to(state, segment, Some(number.clamp(min, max)));
    advance(state, segment)
}

#[derive(Clone, Copy)]
pub struct DateTimeFieldApi(State);

impl DateTimeFieldApi {
    pub fn value(&self) -> Option<DateTime> {
        self.0.value.get_untracked()
    }

    pub fn text(&self) -> String {
        let state = &self.0;
        let values = state.values.get_untracked();
        let typing = state.typing.get_untracked();
        let mut text = String::new();
        for (index, segment) in state.order.value().iter().enumerate() {
            if index > 0 {
                text.push_str(&separator(state.order.value()[index - 1], *segment));
            }
            match (&typing, values[segment.index()]) {
                (Some((typing, buffer)), _) if typing == segment => text.push_str(buffer),
                (_, Some(value)) => text.push_str(&format_segment(*segment, value)),
                (_, None) => text.push_str(segment.placeholder()),
            }
        }
        text
    }
}
