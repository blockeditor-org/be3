use std::rc::Rc;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::datetime::{HourCycle, MINUTES_PER_DAY, Time};
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::NodeId;
use crate::reactive::{
    Callback, Children, ForEach, ItemSize, List, ListChild, Memo, Prop, ReadSignal, RenderFn, WriteSignal, clone,
    component_accessibility, create_effect, create_memo, create_signal, set_component_state,
};
use crate::unstyled;
use crate::unstyled::ButtonHandle;
use crate::unstyled::scroll::ScrollbarStyle;

const PAGE_MINUTES: u32 = 60;
const MORNING: u32 = 9 * 60;
const LEAD_ROWS: usize = 2;

pub struct TimeOptionHandle {
    pub time: Time,
    pub label: String,
    pub selected: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

struct State {
    times: Memo<Vec<u32>>,
    selected: ReadSignal<Option<Time>>,
    set_selected: WriteSignal<Option<Time>>,
    focus: ReadSignal<Option<u32>>,
    set_focus: WriteSignal<Option<u32>>,
    anchor: Memo<u32>,
    on_change: Callback<Time>,
}

type Handle = Rc<State>;

#[component]
pub fn TimeList(
    value: Prop<Option<Time>>,
    #[prop(default = 15)] step_minutes: u32,
    #[prop(default = HourCycle::H24)] hour_cycle: HourCycle,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = 32.0)] row_height: f32,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = ScrollbarStyle::default())] scrollbar: ScrollbarStyle,
    option: RenderFn<TimeOptionHandle>,
    on_change: Callback<Time>,
) -> NodeId {
    let step_minutes = step_minutes.clamp(1, MINUTES_PER_DAY);
    let (selected, set_selected) = create_signal(value.peek());
    create_effect(clone!(set_selected -> move || set_selected.set(value.get())));
    let times = create_memo(clone!(selected -> move || {
        let mut times: Vec<u32> = (0..MINUTES_PER_DAY).step_by(step_minutes as usize).collect();
        if let Some(time) = selected.get()
            && let Err(index) = times.binary_search(&time.minutes())
        {
            times.insert(index, time.minutes());
        }
        times
    }));
    let (focus, set_focus) = create_signal(None);
    let anchor = create_memo(clone!(selected times focus -> move || {
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
    }));
    let shown = create_memo(clone!(times selected -> move || {
        let wanted = selected.get().map_or(MORNING, Time::minutes);
        let index = times.with(|times| times.iter().filter(|minutes| **minutes < wanted).count());
        index.saturating_sub(LEAD_ROWS) as f32 * row_height
    }));
    let state: Handle = Rc::new(State {
        times: times.clone(),
        selected,
        set_selected,
        focus,
        set_focus,
        anchor: anchor.clone(),
        on_change,
    });
    set_component_state(state.clone());
    create_effect(clone!(state -> move || {
        if focused.get() {
            state.set_focus.set(Some(state.anchor.get_untracked()));
        }
    }));
    view! {
        <TimeListBox label>
            <unstyled::Scroll @sizing=ItemSize::Percent(100.0) offset={shown} scrollbar>
                <ForEach keys={times}>
                    {move |minutes: u32| view! {
                        <TimeOption
                            state={state.clone()}
                            minutes
                            hour_cycle
                            face={option.clone()}
                        />
                    }}
                </ForEach>
            </unstyled::Scroll>
        </TimeListBox>
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
    state: Handle,
    minutes: u32,
    hour_cycle: HourCycle,
    face: RenderFn<TimeOptionHandle>,
) -> NodeId {
    let time = Time::from_minutes(minutes);
    let label = time.format(hour_cycle);
    let selected = create_memo(clone!(state -> move || state.selected.get() == Some(time)));
    let tab_stop = create_memo(clone!(state -> move || state.anchor.get() == minutes));
    let focused = create_memo(clone!(state -> move || state.focus.get() == Some(minutes)));
    let accessibility = create_memo(clone!(selected label -> move || {
        let mut node = Node::new(Role::ListBoxOption);
        node.set_label(label.clone());
        node.set_selected(selected.get());
        node
    }));
    let (focus_state, click_state, key_state) = (state.clone(), state.clone(), state.clone());
    view! {
        <unstyled::Button
            tab_stop
            focused
            accessibility
            on_focus_change={move |has_focus: bool| {
                if has_focus {
                    focus_state.set_focus.set(Some(minutes));
                } else if focus_state.focus.get_untracked() == Some(minutes) {
                    focus_state.set_focus.set(None);
                }
            }}
            on_click={move || choose(&click_state, time)}
            on_key={move |press: KeyPress| option_key(&key_state, minutes, press)}
            content={move |button: ButtonHandle| {
                face.call(TimeOptionHandle {
                    time,
                    label,
                    selected,
                    hovered: button.hovered,
                    active: button.active,
                    focused: button.focused,
                })
            }}
        />
    }
}

pub fn time_list_selected(document: &Document, list: NodeId) -> Option<Time> {
    document
        .component_state::<Handle>(list)
        .selected
        .get_untracked()
}

fn choose(state: &State, time: Time) {
    state.set_selected.set(Some(time));
    state.on_change.call(time);
}

fn option_key(state: &State, minutes: u32, press: KeyPress) -> bool {
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
    let next = match press.key {
        Key::ArrowUp => index.saturating_sub(1),
        Key::ArrowDown => (index + 1).min(last),
        Key::PageUp => index.saturating_sub(page),
        Key::PageDown => (index + page).min(last),
        Key::Home => 0,
        Key::End => last,
        _ => return false,
    };
    if press.pressed {
        state.set_focus.set(Some(times[next]));
    }
    true
}
