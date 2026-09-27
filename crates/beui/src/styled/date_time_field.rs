use beui_macros::{component, view};

use crate::base::{Align, Direction, TextAlign};
use crate::color::Color32;
use crate::datetime::{Date, DateTime, HourCycle, Time, Weekday};
use crate::icons::{ICON_CALENDAR_MONTH, ICON_SCHEDULE};
use crate::node::NodeId;
use crate::reactive::{
    Callback, ClickCatcher, Frame, ItemSize, List, Memo, Prop, ReadSignal, Show, Spacer, Text,
    clone, create_effect, create_memo, create_signal,
};
use crate::styled::button::{Button, ButtonVariant};
use crate::styled::calendar::{CALENDAR_WIDTH, Calendar};
use crate::styled::popover::PopoverPanel;
use crate::styled::scroll::scrollbar_style;
use crate::styled::text::IconSized;
use crate::styled::theme::{BORDER_WIDTH, FONT_BODY, ICON_SIZE, RADIUS, ThemeStore, use_theme};
use crate::styled::tooltip::Tooltip;
use crate::unstyled;
use crate::unstyled::{
    DateSegmentHandle, DateTimeParts, PopoverHandle, PopoverTriggerHandle, TimeOptionHandle,
    narrower_than,
};

const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 8.0;
const SEGMENT_PADDING: f32 = 2.0;
const SEGMENT_RADIUS: u8 = 3;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 3.0;
const TRIGGER_PADDING: f32 = 4.0;
const PANEL_SPACING: f32 = 12.0;
const TIME_LIST_WIDTH: f32 = 128.0;
const TIME_LIST_HEIGHT: f32 = 300.0;
const STACKED_TIME_LIST_HEIGHT: f32 = 168.0;
const TIME_ROW_HEIGHT: f32 = 32.0;
const STACK_BREAKPOINT: f32 = 460.0;

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
    let field_report = report.clone();
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius={RADIUS + 3}
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={within.clone()}
        >
            <ClickCatcher on_hover_change={move |inside: bool| set_hovered.set(inside)}>
                <Frame
                    height=HEIGHT
                    color={fill}
                    outline={border}
                    outline_width=BORDER_WIDTH
                    outline_visible=true
                    radius=RADIUS
                    padding_horizontal=PADDING_HORIZONTAL
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                        <unstyled::DateTimeField
                            @sizing=ItemSize::Percent(100.0)
                            value={current.clone()}
                            parts
                            hour_cycle
                            label={label.clone()}
                            disabled={disabled.clone()}
                            on_change={move |next| field_report.call(next)}
                            on_focus_change={move |inside: bool| set_within.set(inside)}
                            segment={|handle: DateSegmentHandle| view! {
                                <SegmentFace handle />
                            }}
                            literal={|text: String| view! {
                                <LiteralFace text />
                            }}
                        />
                        <unstyled::Popover
                            label={picker_label.clone()}
                            disabled={disabled.clone()}
                            trigger={move |handle: PopoverTriggerHandle| view! {
                                <PickerTrigger handle parts label={picker_label} />
                            }}
                        >
                            {move |popover: PopoverHandle| view! {
                                <PopoverPanel>
                                    <PickerPanel
                                        popover
                                        current
                                        parts
                                        hour_cycle
                                        clearable
                                        min
                                        max
                                        first_weekday
                                        step_minutes
                                        report={move |next| report.call(next)}
                                    />
                                </PopoverPanel>
                            }}
                        </unstyled::Popover>
                    </List>
                </Frame>
            </ClickCatcher>
        </Frame>
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
                outline_visible={focused}
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
    current: ReadSignal<Option<DateTime>>,
    parts: DateTimeParts,
    hour_cycle: HourCycle,
    clearable: bool,
    min: Memo<Option<Date>>,
    max: Memo<Option<Date>>,
    first_weekday: Weekday,
    step_minutes: u32,
    report: Callback<Option<DateTime>>,
) -> NodeId {
    let PopoverHandle { open, close } = popover;
    let narrow = narrower_than(STACK_BREAKPOINT);
    let date = create_memo(clone!(current -> move || current.get().map(|value| value.date)));
    let time = create_memo(clone!(current -> move || current.get().map(|value| value.time)));
    let calendar_focused = create_memo(clone!(open -> move || open.get() && parts.has_date()));
    let list_focused = create_memo(clone!(open -> move || open.get() && !parts.has_date()));
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
    let panel_width = create_memo(clone!(narrow -> move || match parts {
        DateTimeParts::Date => CALENDAR_WIDTH,
        DateTimeParts::Time => TIME_LIST_WIDTH,
        DateTimeParts::DateTime if narrow.get() => CALENDAR_WIDTH,
        DateTimeParts::DateTime => CALENDAR_WIDTH + PANEL_SPACING + TIME_LIST_WIDTH,
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
                <unstyled::Stack spacing=PANEL_SPACING narrow>
                    <Show condition=has_date>
                        <Calendar
                            selected={date}
                            min
                            max
                            first_weekday
                            focused={calendar_focused}
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
            outline_visible={focused}
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
