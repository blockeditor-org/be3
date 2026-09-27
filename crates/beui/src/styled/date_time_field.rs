use std::time::Duration;

use beui_macros::{component, view};

use crate::base::{Align, Direction, TextAlign};
use crate::color::Color32;
use crate::datetime::{Date, DateTime, HourCycle, Time, Weekday};
use crate::icons::{ICON_CALENDAR_MONTH, ICON_SCHEDULE};
use crate::input::{CursorIcon, PointerPress};
use crate::node::NodeId;
use crate::reactive::{
    Callback, Children, ClickCallback, ClickCatcher, Frame, ItemSize, List, ListChild, Memo,
    NodeRef, Prop, ReadSignal, Show, Spacer, Text, WriteSignal, clone, component_rect,
    create_effect, create_memo, create_signal, create_timer, focus_ring,
};
use crate::styled::button::{Button, ButtonVariant};
use crate::styled::calendar::{CALENDAR_WIDTH, Calendar};
use crate::styled::popover::{PANEL_PADDING, PopoverPanel};
use crate::styled::scroll::scrollbar_style;
use crate::styled::text::IconSized;
use crate::styled::theme::{BORDER_WIDTH, FONT_BODY, ICON_SIZE, RADIUS, ThemeStore, use_theme};
use crate::styled::tooltip::Tooltip;
use crate::unstyled;
use crate::unstyled::{
    DateDraft, DateSegment, DateSegmentHandle, DateTimeParts, PopoverHandle, PopoverPlacement,
    PopoverTriggerHandle, TimeOptionHandle, narrower_than,
};

const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 8.0;
const SEGMENT_PADDING: f32 = 2.0;
const SEGMENT_RADIUS: u8 = 3;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;
const TRIGGER_PADDING: f32 = 4.0;
const TRIGGER_GAP: f32 = 4.0;
const PANEL_SPACING: f32 = 12.0;
const TIME_LIST_WIDTH: f32 = 128.0;
const TIME_LIST_HEIGHT: f32 = 300.0;
const STACKED_TIME_LIST_HEIGHT: f32 = 168.0;
const TIME_ROW_HEIGHT: f32 = 32.0;
const STACK_BREAKPOINT: f32 = 460.0;

#[derive(Clone)]
struct Field {
    current: ReadSignal<Option<DateTime>>,
    report: Callback<Option<DateTime>>,
    parts: DateTimeParts,
    hour_cycle: HourCycle,
    label: Memo<String>,
    disabled: Memo<bool>,
    width: Memo<f32>,
}

#[component]
pub fn DateTimeField(
    value: Prop<Option<DateTime>>,
    #[prop(default = DateTimeParts::DateTime)] parts: DateTimeParts,
    #[prop(default = HourCycle::H24)] hour_cycle: HourCycle,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] clearable: bool,
    #[prop(default = None)] min: Prop<Option<Date>>,
    #[prop(default = None)] max: Prop<Option<Date>>,
    #[prop(default = Weekday::Monday)] first_weekday: Weekday,
    #[prop(default = 15)] step_minutes: u32,
    on_change: Callback<Option<DateTime>>,
) -> NodeId {
    let (current, set_current) = create_signal(value.peek());
    create_effect(clone!(set_current -> move || set_current.set(value.get())));
    let min = create_memo(move || min.get());
    let max = create_memo(move || max.get());
    let label = create_memo(move || label.get());
    let disabled = create_memo(move || disabled.get());
    let report = Callback::new(clone!(current min max -> move |next: Option<DateTime>| {
        let next = next.map(|next| {
            let date = next.date.clamp_to(min.get_untracked(), max.get_untracked());
            DateTime::new(date, next.time)
        });
        if next != current.get_untracked() {
            set_current.set(next);
            on_change.call(next);
        }
    }));
    let placed = component_rect();
    let width = create_memo(move || placed.get().width());
    let field = Field {
        current: current.clone(),
        report,
        parts,
        hour_cycle,
        label: label.clone(),
        disabled: disabled.clone(),
        width,
    };
    let (open, set_open) = create_signal(false);
    let (from_field, set_from_field) = create_signal(false);
    let (inner_focus, set_inner_focus) = create_signal(None::<DateSegment>);
    let (outer_focus, set_outer_focus) = create_signal(None::<DateSegment>);
    let (last_inner, set_last_inner) = create_signal(None::<DateSegment>);
    let (outer_segment, set_outer_segment) = create_signal(None::<DateSegment>);
    let (within, set_within) = create_signal(false);
    let first = match parts.has_date() {
        true => DateSegment::Year,
        false => DateSegment::Hour,
    };
    let open_at = Callback::new(
        clone!(disabled set_open set_from_field set_inner_focus -> move |segment: DateSegment| {
            if disabled.get_untracked() {
                return;
            }
            set_from_field.set(true);
            set_inner_focus.set(Some(segment));
            set_open.set(true);
        }),
    );
    let pressed = create_timer(clone!(outer_segment open_at -> move || {
        open_at.call(outer_segment.get_untracked().unwrap_or(first));
        None
    }));
    let refocus_trigger = create_memo(clone!(from_field -> move || !from_field.get()));
    let opened = clone!(from_field last_inner set_open set_inner_focus set_outer_focus -> move |now_open: bool| {
        if now_open {
            return;
        }
        set_open.set(false);
        set_inner_focus.set(None);
        if from_field.get_untracked() {
            set_from_field.set(false);
            set_outer_focus.set(Some(last_inner.get_untracked().unwrap_or(first)));
        }
    });
    let picker_label = create_memo(clone!(label -> move || {
        let what = match parts {
            DateTimeParts::Time => "Choose a time",
            DateTimeParts::Date => "Choose a date",
            DateTimeParts::DateTime => "Choose a date and time",
        };
        match label.get() {
            label if label.is_empty() => what.to_owned(),
            label => format!("{what} for {label}"),
        }
    }));
    let anchor = NodeRef::new();
    let theme = use_theme();
    let popover_anchor = anchor.clone();
    let inner_field = field.clone();
    view! {
        <Frame
            @node_ref=&anchor
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius={RADIUS + 3}
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={focus_ring(within.clone())}
        >
            <FieldBox
                field
                focus={outer_focus}
                on_focus_change={move |inside: bool| set_within.set(inside)}
                on_segment_focus={move |segment: Option<DateSegment>| {
                    set_outer_segment.set(segment);
                    if segment.is_some() {
                        set_outer_focus.set(None);
                    }
                }}
                on_open={move |segment: DateSegment| open_at.call(segment)}
                on_press={move || pressed.restart(Duration::ZERO)}
            >
                <unstyled::Popover
                    label={picker_label.clone()}
                    disabled={disabled.clone()}
                    open={open}
                    anchor=popover_anchor
                    placement={PopoverPlacement::Over(PANEL_PADDING as u16)}
                    refocus_trigger
                    on_open_change={opened}
                    trigger={move |handle: PopoverTriggerHandle| view! {
                        <PickerTrigger handle parts label={picker_label} />
                    }}
                >
                    {move |popover: PopoverHandle| view! {
                        <PopoverPanel>
                            <PickerPanel
                                popover
                                field={inner_field}
                                from_field
                                inner_focus
                                set_inner_focus
                                set_last_inner
                                clearable
                                min
                                max
                                first_weekday
                                step_minutes
                            />
                        </PopoverPanel>
                    }}
                </unstyled::Popover>
            </FieldBox>
        </Frame>
    }
}

#[component]
fn FieldBox(
    field: Field,
    focus: Prop<Option<DateSegment>>,
    on_focus_change: Callback<bool>,
    on_segment_focus: Callback<Option<DateSegment>>,
    on_open: Callback<DateSegment>,
    on_draft: Callback<DateDraft>,
    on_press: ClickCallback,
    children: Children<ListChild>,
) -> NodeId {
    let Field {
        current,
        report,
        parts,
        hour_cycle,
        label,
        disabled,
        ..
    } = field;
    let (within, set_within) = create_signal(false);
    let (hovered, set_hovered) = create_signal(false);
    let theme = use_theme();
    let border = create_memo(clone!(theme within disabled -> move || {
        border_color(&theme, disabled.get(), within.get(), hovered.get())
    }));
    let fill = create_memo(clone!(theme disabled -> move || match disabled.get() {
        true => theme.surface.get(),
        false => theme.surface_raised.get(),
    }));
    view! {
        <ClickCatcher on_hover_change={move |inside: bool| set_hovered.set(inside)}>
            <Frame
                height=HEIGHT
                color={fill}
                outline={border}
                outline_width=BORDER_WIDTH
                outline_visible=true
                radius=RADIUS
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                    <ClickCatcher
                        @sizing=ItemSize::Percent(100.0)
                        cursor=CursorIcon::Text
                        on_click_at={move |_: PointerPress| on_press.call()}
                    >
                        <Frame padding_horizontal=PADDING_HORIZONTAL>
                            <unstyled::DateTimeField
                                value={current}
                                parts
                                hour_cycle
                                label
                                disabled
                                focus_segment={focus}
                                on_change={move |next| report.call(next)}
                                on_focus_change={move |inside: bool| {
                                    set_within.set(inside);
                                    on_focus_change.call(inside);
                                }}
                                on_segment_focus={move |segment| on_segment_focus.call(segment)}
                                on_draft={move |draft| on_draft.call(draft)}
                                on_open={move |segment| on_open.call(segment)}
                                segment={|handle: DateSegmentHandle| view! {
                                    <SegmentFace handle />
                                }}
                                literal={|text: String| view! {
                                    <LiteralFace text />
                                }}
                            />
                        </Frame>
                    </ClickCatcher>
                    {children}
                    <Spacer @sizing=ItemSize::Fixed(TRIGGER_GAP) />
                </List>
            </Frame>
        </ClickCatcher>
    }
}

#[component]
fn SegmentFace(handle: DateSegmentHandle) -> NodeId {
    let DateSegmentHandle {
        text,
        placeholder,
        focused,
        disabled,
        ..
    } = handle;
    let theme = use_theme();
    let fill = create_memo(clone!(theme focused -> move || match focused.get() {
        true => theme.accent_soft.get(),
        false => Color32::TRANSPARENT,
    }));
    let ink = create_memo(clone!(theme -> move || {
        match (disabled.get(), placeholder.get(), focused.get()) {
            (true, _, _) | (false, true, false) => theme.text_muted.get(),
            _ => theme.text.get(),
        }
    }));
    view! {
        <Frame color={fill} radius=SEGMENT_RADIUS padding_horizontal=SEGMENT_PADDING>
            <Text string={text} font_size=FONT_BODY color={ink} align=TextAlign::Center />
        </Frame>
    }
}

#[component]
fn LiteralFace(text: String) -> NodeId {
    let theme = use_theme();
    view! {
        <Text string={text} font_size=FONT_BODY color={theme.text_muted.clone()} />
    }
}

#[component]
fn PickerTrigger(
    handle: PopoverTriggerHandle,
    parts: DateTimeParts,
    label: Memo<String>,
) -> NodeId {
    let PopoverTriggerHandle {
        open,
        hovered,
        active,
        focused,
        disabled,
    } = handle;
    let theme = use_theme();
    let fill = create_memo(clone!(theme disabled open -> move || {
        match (disabled.get(), active.get() || open.get(), hovered.get()) {
            (true, _, _) => Color32::TRANSPARENT,
            (false, true, _) => theme.pressed.get(),
            (false, false, true) => theme.hover.get(),
            (false, false, false) => Color32::TRANSPARENT,
        }
    }));
    let ink = create_memo(clone!(theme -> move || match disabled.get() {
        true => theme.text_muted.get(),
        false => theme.text.get(),
    }));
    let glyph = match parts {
        DateTimeParts::Time => ICON_SCHEDULE,
        DateTimeParts::Date | DateTimeParts::DateTime => ICON_CALENDAR_MONTH,
    };
    view! {
        <Tooltip label disabled={open}>
            <Frame
                outline={theme.accent.clone()}
                outline_width=FOCUS_RING_WIDTH
                radius=RADIUS
                outline_offset=1.0
                outline_visible={focus_ring(focused)}
            >
                <Frame
                    color={fill}
                    radius=SEGMENT_RADIUS
                    padding_horizontal=TRIGGER_PADDING
                    padding_vertical=TRIGGER_PADDING
                >
                    <IconSized glyph={glyph.to_owned()} font_size=ICON_SIZE color={ink} />
                </Frame>
            </Frame>
        </Tooltip>
    }
}

#[component]
fn PickerPanel(
    popover: PopoverHandle,
    field: Field,
    from_field: ReadSignal<bool>,
    inner_focus: ReadSignal<Option<DateSegment>>,
    set_inner_focus: WriteSignal<Option<DateSegment>>,
    set_last_inner: WriteSignal<Option<DateSegment>>,
    clearable: bool,
    min: Memo<Option<Date>>,
    max: Memo<Option<Date>>,
    first_weekday: Weekday,
    step_minutes: u32,
) -> NodeId {
    let PopoverHandle { open, close } = popover;
    let Field {
        current,
        report,
        parts,
        hour_cycle,
        width,
        ..
    } = field.clone();
    let (draft, set_draft) = create_signal(DateDraft::default());
    let shown = create_memo(clone!(current draft -> move || {
        let draft = draft.get();
        let held = current.get().map_or_else(Date::today, |value| value.date);
        match (draft.year, draft.month) {
            (None, None) => None,
            (year, month) => Some(Date::new(
                year.unwrap_or(held.year),
                month.unwrap_or(held.month),
                draft.day.unwrap_or(1),
            )),
        }
    }));
    let narrow = narrower_than(STACK_BREAKPOINT);
    let date = create_memo(clone!(current -> move || current.get().map(|value| value.date)));
    let time = create_memo(clone!(current -> move || current.get().map(|value| value.time)));
    let calendar_focused = create_memo(clone!(open from_field -> move || {
        open.get() && !from_field.get() && parts.has_date()
    }));
    let list_focused = create_memo(clone!(open from_field -> move || {
        open.get() && !from_field.get() && !parts.has_date()
    }));
    let list_height = create_memo(
        clone!(narrow -> move || match narrow.get() && parts.has_date() {
            true => STACKED_TIME_LIST_HEIGHT,
            false => TIME_LIST_HEIGHT,
        }),
    );
    let list_width = create_memo(
        clone!(narrow -> move || match narrow.get() && parts.has_date() {
            true => CALENDAR_WIDTH,
            false => TIME_LIST_WIDTH,
        }),
    );
    let panel_width = create_memo(clone!(narrow width -> move || {
        let content = match parts {
            DateTimeParts::Date => CALENDAR_WIDTH,
            DateTimeParts::Time => TIME_LIST_WIDTH,
            DateTimeParts::DateTime if narrow.get() => CALENDAR_WIDTH,
            DateTimeParts::DateTime => CALENDAR_WIDTH + PANEL_SPACING + TIME_LIST_WIDTH,
        };
        content.max(width.get())
    }));
    let pick_date = clone!(current report close -> move |date: Date| {
        let time = current.get_untracked().map_or(Time::MIDNIGHT, |value| value.time);
        report.call(Some(DateTime::new(date, time)));
        if !parts.has_time() {
            close.call(());
        }
    });
    let pick_time = clone!(current report close -> move |time: Time| {
        let date = current.get_untracked().map_or_else(Date::today, |value| value.date);
        report.call(Some(DateTime::new(date, time)));
        close.call(());
    });
    let now = clone!(current report close -> move || {
        let now = DateTime::now();
        let kept = current.get_untracked();
        let next = match parts {
            DateTimeParts::Date => DateTime::new(now.date, kept.map_or(Time::MIDNIGHT, |kept| kept.time)),
            DateTimeParts::Time => DateTime::new(kept.map_or(now.date, |kept| kept.date), now.time),
            DateTimeParts::DateTime => now,
        };
        report.call(Some(next));
        close.call(());
    });
    let clear = clone!(report close -> move || {
        report.call(None);
        close.call(());
    });
    let now_label = match parts {
        DateTimeParts::Time => "Now",
        DateTimeParts::Date | DateTimeParts::DateTime => "Today",
    };
    let has_date = parts.has_date();
    let has_time = parts.has_time();
    view! {
        <Frame width={panel_width}>
            <List spacing=PANEL_SPACING>
                <List direction=Direction::Horizontal spacing=0.0>
                    <Frame width={width}>
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
                            children={view! {}}
                        />
                    </Frame>
                </List>
                <unstyled::Stack spacing=PANEL_SPACING narrow>
                    <Show condition=has_date>
                        <Calendar
                            selected={date}
                            min
                            max
                            first_weekday
                            focused={calendar_focused}
                            show={shown}
                            on_change={pick_date}
                        />
                    </Show>
                    <Show condition=has_time>
                        <Frame width={list_width} height={list_height}>
                            <unstyled::TimeList
                                value={time}
                                step_minutes
                                hour_cycle
                                focused={list_focused}
                                row_height={TIME_ROW_HEIGHT}
                                label="Time"
                                scrollbar={scrollbar_style()}
                                option={|handle: TimeOptionHandle| view! {
                                    <TimeOptionFace handle />
                                }}
                                on_change={pick_time}
                            />
                        </Frame>
                    </Show>
                </unstyled::Stack>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Button label=now_label variant=ButtonVariant::Secondary on_click={now} />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Show condition=clearable>
                        <Button label="Clear" variant=ButtonVariant::Ghost on_click={clear} />
                    </Show>
                </List>
            </List>
        </Frame>
    }
}

#[component]
fn TimeOptionFace(handle: TimeOptionHandle) -> NodeId {
    let TimeOptionHandle {
        label,
        selected,
        hovered,
        active,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let fill = create_memo(clone!(theme selected -> move || {
        match (selected.get(), active.get(), hovered.get()) {
            (true, _, _) => theme.accent.get(),
            (false, true, _) => theme.pressed.get(),
            (false, false, true) => theme.hover.get(),
            (false, false, false) => Color32::TRANSPARENT,
        }
    }));
    let ink = create_memo(clone!(theme -> move || match selected.get() {
        true => theme.on_accent.get(),
        false => theme.text.get(),
    }));
    view! {
        <Frame
            height=TIME_ROW_HEIGHT
            color={fill}
            radius=RADIUS
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            outline_offset=-1.0
            outline_visible={focus_ring(focused)}
            padding_horizontal=10.0
        >
            <Text string={label} font_size=FONT_BODY color={ink} vertical_align=TextAlign::Center />
        </Frame>
    }
}

fn border_color(theme: &ThemeStore, disabled: bool, focused: bool, hovered: bool) -> Color32 {
    if disabled {
        return theme.border.get();
    }
    match (focused, hovered) {
        (true, _) => theme.accent.get(),
        (false, true) => theme.text_muted.get(),
        (false, false) => theme.border.get(),
    }
}
