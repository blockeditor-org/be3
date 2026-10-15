use std::time::Duration;

use beui_macros::{component, view};

use crate::date_time_field::{
    DateDraft, DateSegment, DateSegmentHandle, DateTimeField, DateTimeParts,
};
use crate::datetime::{Date, DateTime, HourCycle, Time};
use crate::popover::{Popover, PopoverHandle, PopoverPlacement, PopoverTriggerHandle};
use beui_core::geometry::Vec2;
use beui_core::input::{CursorIcon, PointerPress};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

pub struct DateTimeBoxHandle {
    pub field: Child,
    pub trigger: Option<Child>,
    pub focused: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub disabled: Memo<bool>,
}

pub struct DateTimeTriggerHandle {
    pub popover: PopoverTriggerHandle,
    pub label: Memo<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DateTimePanelLayout {
    Beside,
    Date,
    Time,
    Paged,
}

#[derive(Clone, Copy)]
pub struct DateTimeCalendarHandle {
    pub date: Memo<Option<Date>>,
    pub shown: Memo<Option<Date>>,
    pub min: Memo<Option<Date>>,
    pub max: Memo<Option<Date>>,
    pub today: Memo<Option<Date>>,
    pub focused: Memo<bool>,
    pub pick: Callback<Date>,
}

#[derive(Clone, Copy)]
pub struct DateTimeTimesHandle {
    pub time: Memo<Option<Time>>,
    pub hour_cycle: HourCycle,
    pub focused: Memo<bool>,
    pub pick: Callback<Time>,
}

pub struct DateTimePanelHandle {
    pub field: Child,
    pub field_width: Memo<f32>,
    pub layout: Memo<DateTimePanelLayout>,
    pub calendar: DateTimeCalendarHandle,
    pub times: DateTimeTimesHandle,
    pub page: ReadSignal<usize>,
    pub show_page: Callback<usize>,
    pub now: ClickCallback,
    pub now_label: &'static str,
    pub clear: ClickCallback,
}

#[derive(Clone, Copy)]
struct Field {
    current: ReadSignal<Option<DateTime>>,
    report: Callback<Option<DateTime>>,
    parts: DateTimeParts,
    hour_cycle: HourCycle,
    label: Memo<String>,
    disabled: Memo<bool>,
    segment: RenderFn<DateSegmentHandle>,
    literal: RenderFn<String>,
    segments: RenderFn<Child>,
    frame: RenderFn<DateTimeBoxHandle>,
}

#[component]
pub fn DateTimePicker(
    value: Prop<Option<DateTime>>,
    #[prop(default = DateTimeParts::DateTime)] parts: DateTimeParts,
    #[prop(default = HourCycle::H24)] hour_cycle: HourCycle,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = None)] min: Prop<Option<Date>>,
    #[prop(default = None)] max: Prop<Option<Date>>,
    #[prop(default = None)] today: Prop<Option<Date>>,
    #[prop(default = false)] paged: Prop<bool>,
    #[prop(default = PopoverPlacement::Below)] placement: PopoverPlacement,
    segment: RenderFn<DateSegmentHandle>,
    literal: RenderFn<String>,
    segments: RenderFn<Child>,
    field: RenderFn<DateTimeBoxHandle>,
    trigger: Render<DateTimeTriggerHandle>,
    panel: Render<DateTimePanelHandle>,
    on_change: Callback<Option<DateTime>>,
) -> NodeId {
    let (current, set_current) = create_signal(value.peek());
    create_effect(move || set_current.set(value.get()));
    let min = create_memo(move || min.get());
    let max = create_memo(move || max.get());
    let today = create_memo(move || today.get());
    let label = create_memo(move || label.get());
    let disabled = create_memo(move || disabled.get());
    let paged = create_memo(move || paged.get());
    let report = Callback::new(move |next: Option<DateTime>| {
        let next = next.map(|next| {
            let date = next.date.clamp_to(min.get_untracked(), max.get_untracked());
            DateTime::new(date, next.time)
        });
        if next != current.get_untracked() {
            set_current.set(next);
            on_change.call(next);
        }
    });
    view! {
        <LayoutBuilder>
            {move |size: ReadSignal<Vec2>| {
                let width = create_memo(move || size.get().x);
                let field = Field {
                    current,
                    report,
                    parts,
                    hour_cycle,
                    label,
                    disabled,
                    segment,
                    literal,
                    segments,
                    frame: field,
                };
                let (open, set_open) = create_signal(false);
                let (from_field, set_from_field) = create_signal(false);
                let (inner_focus, set_inner_focus) = create_signal(None::<DateSegment>);
                let (outer_focus, set_outer_focus) = create_signal(None::<DateSegment>);
                let (last_inner, set_last_inner) = create_signal(None::<DateSegment>);
                let (outer_segment, set_outer_segment) = create_signal(None::<DateSegment>);
                let first = match parts.has_date() {
                    true => DateSegment::Year,
                    false => DateSegment::Hour,
                };
                let open_at = Callback::new(
                    move |segment: DateSegment| {
                        if disabled.get_untracked() {
                            return;
                        }
                        set_from_field.set(true);
                        set_inner_focus.set(Some(segment));
                        set_open.set(true);
                    },
                );
                let pressed = create_timer(move || {
                    open_at.call(outer_segment.get_untracked().unwrap_or(first));
                    None
                });
                let refocus_trigger = create_memo(move || !from_field.get());
                let opened = move |now_open: bool| {
                    if now_open {
                        return;
                    }
                    set_open.set(false);
                    set_inner_focus.set(None);
                    if from_field.get_untracked() {
                        set_from_field.set(false);
                        set_outer_focus.set(Some(last_inner.get_untracked().unwrap_or(first)));
                    }
                };
                let picker_label = create_memo(move || {
                    let what = match parts {
                        DateTimeParts::Time => "Choose a time",
                        DateTimeParts::Date => "Choose a date",
                        DateTimeParts::DateTime => "Choose a date and time",
                    };
                    match label.get() {
                        label if label.is_empty() => what.to_owned(),
                        label => format!("{what} for {label}"),
                    }
                });
                let anchor = NodeRef::new();
                let popover_anchor = anchor;
                let trigger_label = picker_label;
                view! {
                    <Frame @node_ref=&anchor>
                        <FieldBox
                            field
                            focus={outer_focus}
                            trigger={view! {
                                <Popover
                                    label={picker_label}
                                    disabled={disabled}
                                    open={open}
                                    anchor=popover_anchor
                                    placement
                                    refocus_trigger
                                    on_open_change={opened}
                                    trigger={move |handle: PopoverTriggerHandle| trigger.call(DateTimeTriggerHandle {
                                        popover: handle,
                                        label: trigger_label,
                                    })}
                                >
                                    {move |popover: PopoverHandle| view! {
                                        <PickerPanel
                                            popover
                                            field={field}
                                            width
                                            from_field
                                            inner_focus
                                            set_inner_focus
                                            set_last_inner
                                            paged
                                            min
                                            max
                                            today
                                            panel
                                        />
                                    }}
                                </Popover>
                            }}
                            on_segment_focus={move |segment: Option<DateSegment>| {
                                set_outer_segment.set(segment);
                                if segment.is_some() {
                                    set_outer_focus.set(None);
                                }
                            }}
                            on_open={open_at}
                            on_press={move || pressed.restart(Duration::ZERO)}
                        />
                    </Frame>
                }
            }}
        </LayoutBuilder>
    }
}

#[component]
fn FieldBox(
    field: Field,
    focus: Prop<Option<DateSegment>>,
    #[prop(default = None)] trigger: Option<Child>,
    on_segment_focus: Callback<Option<DateSegment>>,
    on_open: Callback<DateSegment>,
    on_draft: Callback<DateDraft>,
    on_press: ClickCallback,
) -> NodeId {
    let Field {
        current,
        report,
        parts,
        hour_cycle,
        label,
        disabled,
        segment,
        literal,
        segments,
        frame,
    } = field;
    let (within, set_within) = create_signal(false);
    let (hovered, set_hovered) = create_signal(false);
    let field_disabled = disabled;
    view! {
        <Interactive on_hover_change={move |inside: bool| set_hovered.set(inside)}>
            {frame.call(DateTimeBoxHandle {
                field: view! {
                    <Interactive
                        cursor=CursorIcon::Text
                        on_click_at={move |_: PointerPress| on_press.call()}
                    >
                        {segments.call(view! {
                            <DateTimeField
                                value={current}
                                parts
                                hour_cycle
                                label
                                disabled={field_disabled}
                                focus_segment={focus}
                                on_change={report}
                                on_focus_change={move |inside: bool| set_within.set(inside)}
                                on_segment_focus={on_segment_focus}
                                on_draft={on_draft}
                                on_open={on_open}
                                segment
                                literal
                            />
                        })}
                    </Interactive>
                },
                trigger,
                focused: within,
                hovered,
                disabled,
            })}
        </Interactive>
    }
}

#[component]
fn PickerPanel(
    popover: PopoverHandle,
    field: Field,
    width: Memo<f32>,
    from_field: ReadSignal<bool>,
    inner_focus: ReadSignal<Option<DateSegment>>,
    set_inner_focus: WriteSignal<Option<DateSegment>>,
    set_last_inner: WriteSignal<Option<DateSegment>>,
    paged: Memo<bool>,
    min: Memo<Option<Date>>,
    max: Memo<Option<Date>>,
    today: Memo<Option<Date>>,
    panel: Render<DateTimePanelHandle>,
) -> NodeId {
    let PopoverHandle { open, close } = popover;
    let Field {
        current,
        report,
        parts,
        hour_cycle,
        ..
    } = field;
    let (draft, set_draft) = create_signal(DateDraft::default());
    let shown = create_memo(move || {
        let draft = draft.get();
        let held = current
            .get()
            .map(|value| value.date)
            .or_else(|| today.get())
            .unwrap_or_else(Date::today);
        match (draft.year, draft.month) {
            (None, None) => None,
            (year, month) => Some(Date::new(
                year.unwrap_or(held.year),
                month.unwrap_or(held.month),
                draft.day.unwrap_or(1),
            )),
        }
    });
    let (tab, set_tab) = create_signal(0usize);
    create_effect(move || {
        if open.get() {
            let timed = inner_focus.get_untracked().is_some_and(|segment| {
                matches!(
                    segment,
                    DateSegment::Hour | DateSegment::Minute | DateSegment::Period
                )
            });
            set_tab.set(usize::from(timed));
        }
    });
    let date = create_memo(move || current.get().map(|value| value.date));
    let time = create_memo(move || current.get().map(|value| value.time));
    let calendar_focused = create_memo(move || open.get() && !from_field.get() && parts.has_date());
    let list_focused = create_memo(move || {
        open.get() && !from_field.get() && (!parts.has_date() || tab.get() == 1)
    });
    let pick_date = Callback::new(move |date: Date| {
        let time = current
            .get_untracked()
            .map_or(Time::MIDNIGHT, |value| value.time);
        report.call(Some(DateTime::new(date, time)));
        match parts {
            DateTimeParts::Date => close.call(()),
            DateTimeParts::DateTime if paged.get_untracked() => set_tab.set(1),
            DateTimeParts::DateTime | DateTimeParts::Time => {}
        }
    });
    let pick_time = Callback::new(move |time: Time| {
        let date = current
            .get_untracked()
            .map(|value| value.date)
            .or_else(|| today.get_untracked())
            .unwrap_or_else(Date::today);
        report.call(Some(DateTime::new(date, time)));
        close.call(());
    });
    let now = ClickCallback::new(move || {
        let now = DateTime::now();
        let kept = current.get_untracked();
        let next = match parts {
            DateTimeParts::Date => {
                DateTime::new(now.date, kept.map_or(Time::MIDNIGHT, |kept| kept.time))
            }
            DateTimeParts::Time => DateTime::new(kept.map_or(now.date, |kept| kept.date), now.time),
            DateTimeParts::DateTime => now,
        };
        report.call(Some(next));
        close.call(());
    });
    let clear = ClickCallback::new(move || {
        report.call(None);
        close.call(());
    });
    let layout = create_memo(move || match parts {
        DateTimeParts::Date => DateTimePanelLayout::Date,
        DateTimeParts::Time => DateTimePanelLayout::Time,
        DateTimeParts::DateTime if paged.get() => DateTimePanelLayout::Paged,
        DateTimeParts::DateTime => DateTimePanelLayout::Beside,
    });
    let now_label = match parts {
        DateTimeParts::Time => "Now",
        DateTimeParts::Date | DateTimeParts::DateTime => "Today",
    };
    view! {
        <Frame>
            {panel.call(DateTimePanelHandle {
                field: view! {
                    <FieldBox
                        field
                        focus={inner_focus}
                        on_segment_focus={move |segment: Option<DateSegment>| {
                            if segment.is_some() {
                                set_last_inner.set(segment);
                                set_inner_focus.set(None);
                            }
                        }}
                        on_draft={move |draft| set_draft.set(draft)}
                    />
                },
                field_width: width,
                layout,
                calendar: DateTimeCalendarHandle {
                    date,
                    shown,
                    min,
                    max,
                    today,
                    focused: calendar_focused,
                    pick: pick_date,
                },
                times: DateTimeTimesHandle {
                    time,
                    hour_cycle,
                    focused: list_focused,
                    pick: pick_time,
                },
                page: tab,
                show_page: Callback::new(move |page: usize| set_tab.set(page)),
                now,
                now_label,
                clear,
            })}
        </Frame>
    }
}
