use std::time::Duration;

use beui_macros::{component, view};

use crate::button::{Button, ButtonVariant};
use crate::calendar::{CALENDAR_WIDTH, Calendar};
use crate::popover::{PANEL_PADDING, PopoverPanel};
use crate::scroll::scrollbar_style;
use crate::tabs::Tabs;
use crate::text::IconSized;
use crate::theme::{BORDER_WIDTH, FONT_BODY, ICON_SIZE, RADIUS, ThemeStore, use_theme};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::datetime::{Date, DateTime, HourCycle, Time, Weekday};
use beui_components_unstyled::{
    ChoiceOption, DateDraft, DateSegment, DateSegmentHandle, DateTimeParts, PopoverHandle,
    PopoverPlacement, PopoverTriggerHandle, TimeOptionHandle, narrower_than,
};
use beui_core::base::{Align, Direction, Justify, TextAlign};
use beui_core::color::Color32;
use beui_core::icons::{ICON_CALENDAR_MONTH, ICON_SCHEDULE};
use beui_core::input::{CursorIcon, PointerPress};
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, Child, Children, ClickCallback, Dynamic, Frame, Interactive, ItemSize, List,
    ListChild, Memo, NodeRef, Prop, ReadSignal, Show, Text, WriteSignal, clone, component_rect,
    create_effect, create_memo, create_signal, create_timer, focus_ring,
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
const TIME_GRID_HEIGHT: f32 = 264.0;
const TIME_GRID_MIN_WIDTH: f32 = 280.0;
const TIME_GRID_SPACING: f32 = 4.0;
const MAX_GRID_COLUMNS: u32 = 4;
const CALENDAR_MAX_WIDTH: f32 = 420.0;
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
    #[prop(default = None)] today: Prop<Option<Date>>,
    #[prop(default = Weekday::Monday)] first_weekday: Weekday,
    #[prop(default = 15)] step_minutes: u32,
    on_change: Callback<Option<DateTime>>,
) -> NodeId {
    let (current, set_current) = create_signal(value.peek());
    create_effect(clone!(set_current -> move || set_current.set(value.get())));
    let min = create_memo(move || min.get());
    let max = create_memo(move || max.get());
    let today = create_memo(move || today.get());
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
                                today
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
        <Interactive on_hover_change={move |inside: bool| set_hovered.set(inside)}>
            <Frame
                height=HEIGHT
                color={fill}
                outline={border}
                outline_width=BORDER_WIDTH
                outline_visible=true
                radius=RADIUS
                padding_right=TRIGGER_GAP
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                    <Interactive
                        @sizing=ItemSize::Percent(100.0)
                        cursor=CursorIcon::Text
                        on_click_at={move |_: PointerPress| on_press.call()}
                    >
                        <Frame height=HEIGHT padding_horizontal=PADDING_HORIZONTAL>
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
                    </Interactive>
                    {children}
                </List>
            </Frame>
        </Interactive>
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
        <Frame height_fraction=1.0 align_vertical=Align::Center>
            <Frame color={fill} radius=SEGMENT_RADIUS padding_horizontal=SEGMENT_PADDING>
                <Text string={text} font_size=FONT_BODY color={ink} align=TextAlign::Center />
            </Frame>
        </Frame>
    }
}

#[component]
fn LiteralFace(text: String) -> NodeId {
    let theme = use_theme();
    view! {
        <Text
            string={text}
            font_size=FONT_BODY
            color={theme.text_muted.clone()}
            vertical_align=TextAlign::Center
        />
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
    today: Memo<Option<Date>>,
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
    let shown = create_memo(clone!(current draft today -> move || {
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
    }));
    let narrow = narrower_than(STACK_BREAKPOINT);
    let layout = create_memo(clone!(narrow -> move || match parts {
        DateTimeParts::Date => PanelLayout::Date,
        DateTimeParts::Time => PanelLayout::Time,
        DateTimeParts::DateTime if narrow.get() => PanelLayout::Paged,
        DateTimeParts::DateTime => PanelLayout::Beside,
    }));
    let (tab, set_tab) = create_signal(0usize);
    create_effect(clone!(open inner_focus set_tab -> move || {
        if open.get() {
            let timed = inner_focus.get_untracked().is_some_and(|segment| {
                matches!(segment, DateSegment::Hour | DateSegment::Minute | DateSegment::Period)
            });
            set_tab.set(usize::from(timed));
        }
    }));
    let panel_width = create_memo(clone!(layout width -> move || {
        let least = match layout.get() {
            PanelLayout::Date | PanelLayout::Paged => CALENDAR_WIDTH,
            PanelLayout::Time => TIME_GRID_MIN_WIDTH,
            PanelLayout::Beside => CALENDAR_WIDTH + PANEL_SPACING + TIME_LIST_WIDTH,
        };
        least.max(width.get())
    }));
    let date = create_memo(clone!(current -> move || current.get().map(|value| value.date)));
    let time = create_memo(clone!(current -> move || current.get().map(|value| value.time)));
    let calendar_focused = create_memo(clone!(open from_field -> move || {
        open.get() && !from_field.get() && parts.has_date()
    }));
    let list_focused = create_memo(clone!(open from_field tab -> move || {
        open.get() && !from_field.get() && (!parts.has_date() || tab.get() == 1)
    }));
    let pick_date = Callback::new(
        clone!(current report close layout set_tab -> move |date: Date| {
            let time = current.get_untracked().map_or(Time::MIDNIGHT, |value| value.time);
            report.call(Some(DateTime::new(date, time)));
            match layout.get_untracked() {
                PanelLayout::Date => close.call(()),
                PanelLayout::Paged => set_tab.set(1),
                PanelLayout::Beside | PanelLayout::Time => {}
            }
        }),
    );
    let pick_time = Callback::new(clone!(current report close today -> move |time: Time| {
        let date = current
            .get_untracked()
            .map(|value| value.date)
            .or_else(|| today.get_untracked())
            .unwrap_or_else(Date::today);
        report.call(Some(DateTime::new(date, time)));
        close.call(());
    }));
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
    let pieces = Pieces {
        date,
        time,
        shown,
        min,
        max,
        today,
        first_weekday,
        hour_cycle,
        step_minutes,
        calendar_focused,
        list_focused,
        pick_date,
        pick_time,
        width: panel_width.clone(),
    };
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
                <Dynamic value={layout}>
                    {move |shape: PanelLayout| {
                        let pieces = pieces.clone();
                        let (tab, set_tab) = (tab.clone(), set_tab.clone());
                        view! {
                            <PanelBody shape pieces tab set_tab />
                        }
                    }}
                </Dynamic>
                <List
                    direction=Direction::Horizontal
                    align=Align::Center
                    justify=Justify::SpaceBetween
                    spacing=8.0
                >
                    <Button label=now_label variant=ButtonVariant::Secondary on_click={now} />
                    <Show condition=clearable>
                        <Button
                            label="Clear"
                            variant=ButtonVariant::Ghost
                            on_click={clear.clone()}
                        />
                    </Show>
                </List>
            </List>
        </Frame>
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PanelLayout {
    Beside,
    Date,
    Time,
    Paged,
}

#[derive(Clone)]
struct Pieces {
    date: Memo<Option<Date>>,
    time: Memo<Option<Time>>,
    shown: Memo<Option<Date>>,
    min: Memo<Option<Date>>,
    max: Memo<Option<Date>>,
    today: Memo<Option<Date>>,
    first_weekday: Weekday,
    hour_cycle: HourCycle,
    step_minutes: u32,
    calendar_focused: Memo<bool>,
    list_focused: Memo<bool>,
    pick_date: Callback<Date>,
    pick_time: Callback<Time>,
    width: Memo<f32>,
}

#[component]
fn PanelBody(
    shape: PanelLayout,
    pieces: Pieces,
    tab: ReadSignal<usize>,
    set_tab: WriteSignal<usize>,
) -> NodeId {
    let width = pieces.width.clone();
    let beside = create_memo(clone!(width -> move || {
        (width.get() - PANEL_SPACING - TIME_LIST_WIDTH).min(CALENDAR_MAX_WIDTH)
    }));
    let alone = create_memo(move || width.get().min(CALENDAR_MAX_WIDTH));
    let columns = grid_columns(pieces.step_minutes);
    let dates = create_memo(clone!(tab -> move || tab.get() == 0));
    let times = create_memo(clone!(tab -> move || tab.get() == 1));
    let (beside_pieces, date_pieces, time_pieces, tab_dates, tab_times) = (
        pieces.clone(),
        pieces.clone(),
        pieces.clone(),
        pieces.clone(),
        pieces,
    );
    let tab_alone = alone.clone();
    view! {
        <List spacing=PANEL_SPACING>
            <Show condition={shape == PanelLayout::Beside}>
                {move || clone!(beside_pieces beside -> view! {
                    <List direction=Direction::Horizontal spacing=PANEL_SPACING>
                        <CalendarPane pieces={beside_pieces.clone()} width={beside} />
                        <TimePane
                            @sizing=ItemSize::Percent(100.0)
                            pieces={beside_pieces}
                            columns=1
                            height=TIME_LIST_HEIGHT
                        />
                    </List>
                })}
            </Show>
            <Show condition={shape == PanelLayout::Date}>
                {move || clone!(date_pieces alone -> view! {
                    <Centred>
                        <CalendarPane pieces={date_pieces} width={alone} />
                    </Centred>
                })}
            </Show>
            <Show condition={shape == PanelLayout::Time}>
                <TimePane pieces={time_pieces.clone()} columns height=TIME_GRID_HEIGHT />
            </Show>
            <Show condition={shape == PanelLayout::Paged}>
                {move || clone!(tab set_tab dates times tab_dates tab_alone tab_times -> view! {
                    <List spacing=PANEL_SPACING>
                        <Tabs
                            options={view! {
                                <ChoiceOption label="Date" />
                                <ChoiceOption label="Time" />
                            }}
                            selected={tab}
                            on_change={move |chosen: usize| set_tab.set(chosen)}
                        />
                        <Show condition={dates}>
                            {move || clone!(tab_dates tab_alone -> view! {
                                <Centred>
                                    <CalendarPane pieces={tab_dates} width={tab_alone} />
                                </Centred>
                            })}
                        </Show>
                        <Show condition={times}>
                            <TimePane pieces={tab_times.clone()} columns height=TIME_GRID_HEIGHT />
                        </Show>
                    </List>
                })}
            </Show>
        </List>
    }
}

#[component]
fn Centred(children: Child) -> NodeId {
    view! {
        <Frame width_fraction=1.0 align_horizontal=Align::Center>{children}</Frame>
    }
}

#[component]
fn CalendarPane(pieces: Pieces, width: Memo<f32>) -> NodeId {
    let Pieces {
        date,
        shown,
        min,
        max,
        today,
        first_weekday,
        calendar_focused,
        pick_date,
        ..
    } = pieces;
    view! {
        <Calendar
            selected={date}
            min
            max
            today
            first_weekday
            focused={calendar_focused}
            show={shown}
            width
            on_change={move |date| pick_date.call(date)}
        />
    }
}

#[component]
fn TimePane(pieces: Pieces, columns: usize, height: f32) -> NodeId {
    let Pieces {
        time,
        hour_cycle,
        step_minutes,
        list_focused,
        pick_time,
        ..
    } = pieces;
    let centred = columns > 1;
    view! {
        <Frame height>
            <unstyled::TimeList
                value={time}
                step_minutes
                hour_cycle
                focused={list_focused}
                row_height={TIME_ROW_HEIGHT}
                columns
                spacing={if centred { TIME_GRID_SPACING } else { 0.0 }}
                label="Time"
                scrollbar={scrollbar_style()}
                option={move |handle: TimeOptionHandle| view! {
                    <TimeOptionFace handle centred />
                }}
                on_change={move |time| pick_time.call(time)}
            />
        </Frame>
    }
}

fn grid_columns(step_minutes: u32) -> usize {
    match step_minutes {
        step if step > 0 && step < 60 && 60 % step == 0 => {
            (60 / step).min(MAX_GRID_COLUMNS) as usize
        }
        _ => 1,
    }
}

#[component]
fn TimeOptionFace(handle: TimeOptionHandle, centred: bool) -> NodeId {
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
            <Text
                string={label}
                font_size=FONT_BODY
                color={ink}
                align={if centred { TextAlign::Center } else { TextAlign::Start }}
                vertical_align=TextAlign::Center
            />
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
