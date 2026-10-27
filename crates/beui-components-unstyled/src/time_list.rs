use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate as unstyled;
use crate::ButtonHandle;
use crate::datetime::{HourCycle, MINUTES_PER_DAY, Time};
use crate::scroll::ScrollbarStyle;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Direction;
use beui_view::types::ItemSize;
use beui_view::types::{Key, KeyPress};

const PAGE_MINUTES: u32 = 60;
const MORNING: u32 = 9 * 60;
const LEAD_ROWS: usize = 2;
const PAGE_ROWS: usize = 4;

pub struct TimeOptionHandle {
    pub time: Time,
    pub label: String,
    pub selected: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

#[derive(Clone, Copy)]
struct State {
    times: Memo<Vec<u32>>,
    selected: Binding<Option<Time>>,
    focus: Signal<Option<u32>>,
    anchor: Memo<u32>,
    columns: usize,
}

#[component]
pub fn TimeList(
    #[prop(default = ComponentRef::new())] api: ComponentRef<TimeListApi>,
    #[prop(default = None)] value: Binding<Option<Time>>,
    #[prop(default = 15)] step_minutes: u32,
    #[prop(default = HourCycle::H24)] hour_cycle: HourCycle,
    #[prop(default = false)] focused: Prop<bool>,
    row_height: f32,
    #[prop(default = 1)] max_columns: usize,
    #[prop(default = 0.0)] spacing: f32,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = ScrollbarStyle::default())] scrollbar: ScrollbarStyle,
    option: RenderFn<TimeOptionHandle>,
) -> NodeId {
    let step_minutes = step_minutes.clamp(1, MINUTES_PER_DAY);
    let columns = grid_columns(step_minutes, max_columns);
    let selected = value;
    let times = create_memo(move || {
        let mut times: Vec<u32> = (0..MINUTES_PER_DAY)
            .step_by(step_minutes as usize)
            .collect();
        if columns == 1
            && let Some(time) = selected.get()
            && let Err(index) = times.binary_search(&time.minutes())
        {
            times.insert(index, time.minutes());
        }
        times
    });
    let focus = create_signal(None);
    let anchor = create_memo(move || {
        if let Some(focused) = focus.get() {
            return focused;
        }
        let wanted = selected.get().map_or(MORNING, Time::minutes);
        times.with(|times| {
            times
                .iter()
                .rev()
                .find(|minutes| **minutes <= wanted)
                .or(times.first())
                .copied()
                .unwrap_or(0)
        })
    });
    let shown = create_memo(move || {
        let wanted = selected.get().map_or(MORNING, Time::minutes);
        let index = times.with(|times| times.iter().filter(|minutes| **minutes < wanted).count());
        (index / columns).saturating_sub(LEAD_ROWS) as f32 * (row_height + spacing)
    });
    let state = State {
        times,
        selected,
        focus,
        anchor,
        columns,
    };
    api.fill(TimeListApi(state));
    create_effect(move || {
        if focused.get() {
            state.focus.set(Some(state.anchor.get_untracked()));
        }
    });
    let rows =
        create_memo(move || (0..times.with(Vec::len).div_ceil(columns)).collect::<Vec<usize>>());
    view! {
        <TimeListBox label>
            <unstyled::Scroll @sizing=ItemSize::Percent(100.0) offset={shown} scrollbar>
                <List spacing>
                    <ForEach keys={rows}>|row: usize|
                        <TimeRow state={state} row spacing hour_cycle face={option} />
                    </ForEach>
                </List>
            </unstyled::Scroll>
        </TimeListBox>
    }
}

#[component]
fn TimeRow(
    state: State,
    row: usize,
    spacing: f32,
    hour_cycle: HourCycle,
    face: RenderFn<TimeOptionHandle>,
) -> NodeId {
    let columns = state.columns;
    let times = state.times;
    let cells = create_memo(move || {
        times.with(|times| {
            times
                .iter()
                .skip(row * columns)
                .take(columns)
                .copied()
                .collect::<Vec<u32>>()
        })
    });
    view! {
        <List direction=Direction::Horizontal spacing>
            <ForEach keys={cells}>|minutes: u32|
                <TimeOption
                    @sizing=ItemSize::Percent(100.0)
                    state={state}
                    minutes
                    hour_cycle
                    face={face}
                />
            </ForEach>
        </List>
    }
}

#[component]
fn TimeListBox(label: Prop<String>, children: Children<ListChild>) -> NodeId {
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::ListBox);
        let label = label.get();
        if !label.is_empty() {
            node.set_label(label);
        }
        node
    }));
    view! {
        <List spacing=0.0 children />
    }
}

#[component]
fn TimeOption(
    state: State,
    minutes: u32,
    hour_cycle: HourCycle,
    face: RenderFn<TimeOptionHandle>,
) -> NodeId {
    let time = Time::from_minutes(minutes);
    let label = time.format(hour_cycle);
    let selected = create_memo(move || state.selected.get() == Some(time));
    let tab_stop = create_memo(move || state.anchor.get() == minutes);
    let focused = create_memo(move || state.focus.get() == Some(minutes));
    let accessibility = create_memo({
        let label = label.clone();
        move || {
            let mut node = Node::new(Role::ListBoxOption);
            node.set_label(label.clone());
            node.set_selected(selected.get());
            node
        }
    });
    view! {
        <unstyled::Button
            tab_stop
            focused
            accessibility
            on_focus_change={move |has_focus: bool| {
                if has_focus {
                    state.focus.set(Some(minutes));
                } else if state.focus.get_untracked() == Some(minutes) {
                    state.focus.set(None);
                }
            }}
            on_click={move || choose(state, time)}
            on_key={move |press: KeyPress| option_key(state, minutes, press)}
        >|button: ButtonHandle| {
            face.call(TimeOptionHandle {
                time,
                label,
                selected,
                hovered: button.hovered,
                active: button.active,
                focused: button.focused,
            })
        }</unstyled::Button>
    }
}

fn choose(state: State, time: Time) {
    state.selected.update(|selected| *selected = Some(time));
}

fn option_key(state: State, minutes: u32, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let times = state.times.get_untracked();
    let Some(index) = times.iter().position(|each| *each == minutes) else {
        return false;
    };
    let last = times.len() - 1;
    let page = times
        .iter()
        .position(|each| *each >= minutes + PAGE_MINUTES)
        .map_or(last, |later| later - index)
        .max(1);
    let columns = state.columns;
    let page = match columns {
        1 => page,
        _ => columns * PAGE_ROWS,
    };
    let next = match press.key {
        Key::ArrowLeft if columns > 1 => index.saturating_sub(1),
        Key::ArrowRight if columns > 1 => (index + 1).min(last),
        Key::ArrowUp => index.saturating_sub(columns),
        Key::ArrowDown => (index + columns).min(last),
        Key::PageUp => index.saturating_sub(page),
        Key::PageDown => (index + page).min(last),
        Key::Home => 0,
        Key::End => last,
        _ => return false,
    };
    if press.pressed {
        state.focus.set(Some(times[next]));
    }
    true
}

pub fn grid_columns(step_minutes: u32, max_columns: usize) -> usize {
    match step_minutes {
        step if step > 0 && step < 60 && 60 % step == 0 => (60 / step as usize).min(max_columns),
        _ => 1,
    }
    .max(1)
}

#[derive(Clone, Copy)]
pub struct TimeListApi(State);

impl TimeListApi {
    pub fn selected(&self) -> Option<Time> {
        self.0.selected.get_untracked()
    }
}
