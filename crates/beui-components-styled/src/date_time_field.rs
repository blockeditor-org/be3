use beui_macros::{component, view};

use crate::button::{Button, ButtonVariant};
use crate::calendar::{CALENDAR_WIDTH, Calendar};
use crate::popover::{PANEL_PADDING, PopoverPanel};
use crate::scroll::scrollbar_style;
use crate::tabs::Tabs;
use crate::text::IconSized;
use crate::theme::{BORDER_WIDTH, FONT_BODY, ICON_SIZE, RADIUS, field_border, use_theme};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::datetime::{Date, DateTime, HourCycle, Weekday};
use beui_components_unstyled::{
    ButtonHandle, ChoiceOption, DateSegmentHandle, DateTimeBoxHandle, DateTimeCalendarHandle,
    DateTimePanelHandle, DateTimePanelLayout, DateTimeParts, DateTimeTimesHandle,
    DateTimeTriggerHandle, PopoverPlacement, PopoverTriggerHandle, TimeOptionHandle, grid_columns,
    narrower_than,
};
use beui_core::base::{Align, Direction, Justify, TextAlign};
use beui_core::color::Color32;
use beui_core::icons::{ICON_CALENDAR_MONTH, ICON_SCHEDULE};
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, Child, Dynamic, Frame, ItemSize, List, Memo, Prop, ReadSignal, Show, Text, clone,
    create_memo, focus_ring,
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
const MAX_GRID_COLUMNS: usize = 4;
const CALENDAR_MAX_WIDTH: f32 = 420.0;
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
    #[prop(default = None)] today: Prop<Option<Date>>,
    #[prop(default = Weekday::Monday)] first_weekday: Weekday,
    #[prop(default = 15)] step_minutes: u32,
    on_change: Callback<Option<DateTime>>,
) -> NodeId {
    let narrow = narrower_than(STACK_BREAKPOINT);
    view! {
        <unstyled::DateTimePicker
            value
            parts
            hour_cycle
            label
            disabled
            min
            max
            today
            paged={narrow}
            placement={PopoverPlacement::Over(PANEL_PADDING as u16)}
            segment={|handle: DateSegmentHandle| view! {
                <SegmentFace handle />
            }}
            literal={|text: String| view! {
                <LiteralFace text />
            }}
            segments={|field: Child| view! {
                <Frame height=HEIGHT padding_horizontal=PADDING_HORIZONTAL>{field}</Frame>
            }}
            field={|handle: DateTimeBoxHandle| view! {
                <FieldBox handle />
            }}
            trigger={move |handle: DateTimeTriggerHandle| view! {
                <PickerTrigger handle parts />
            }}
            panel={move |handle: DateTimePanelHandle| view! {
                <PopoverPanel>
                    <PickerPanel handle clearable first_weekday step_minutes />
                </PopoverPanel>
            }}
            on_change={move |next| on_change.call(next)}
        />
    }
}

#[component]
fn FieldBox(handle: DateTimeBoxHandle) -> NodeId {
    let DateTimeBoxHandle {
        field,
        trigger,
        focused,
        hovered,
        disabled,
    } = handle;
    let theme = use_theme();
    let border = create_memo(clone!(theme focused disabled -> move || {
        field_border(&theme, disabled.get(), focused.get(), hovered.get())
    }));
    let fill = create_memo(clone!(theme disabled -> move || match disabled.get() {
        true => theme.surface.get(),
        false => theme.surface_raised.get(),
    }));
    let outer = trigger.is_some();
    let keyboard = focus_ring(focused);
    let ring = create_memo(move || outer && keyboard.get());
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius={RADIUS + 3}
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={ring}
        >
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
                    {field} @sizing=ItemSize::Percent(100.0)
                    {trigger}
                </List>
            </Frame>
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
fn PickerTrigger(handle: DateTimeTriggerHandle, parts: DateTimeParts) -> NodeId {
    let DateTimeTriggerHandle {
        popover:
            PopoverTriggerHandle {
                open,
                button:
                    ButtonHandle {
                        hovered,
                        active,
                        focused,
                        disabled,
                        ..
                    },
            },
        label,
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
fn PickerPanel(handle: DateTimePanelHandle, clearable: bool, first_weekday: Weekday, step_minutes: u32) -> NodeId {
    let DateTimePanelHandle {
        field,
        field_width,
        layout,
        calendar,
        times,
        page,
        show_page,
        now,
        now_label,
        clear,
    } = handle;
    let panel_width = create_memo(clone!(layout field_width -> move || {
        let least = match layout.get() {
            DateTimePanelLayout::Date | DateTimePanelLayout::Paged => CALENDAR_WIDTH,
            DateTimePanelLayout::Time => TIME_GRID_MIN_WIDTH,
            DateTimePanelLayout::Beside => CALENDAR_WIDTH + PANEL_SPACING + TIME_LIST_WIDTH,
        };
        least.max(field_width.get())
    }));
    let width = panel_width.clone();
    view! {
        <Frame width={panel_width}>
            <List spacing=PANEL_SPACING>
                <List direction=Direction::Horizontal spacing=0.0>
                    <Frame width={field_width}>{field}</Frame>
                </List>
                <Dynamic value={layout}>
                    {move |shape: DateTimePanelLayout| {
                        let (calendar, times) = (calendar.clone(), times.clone());
                        let (page, show_page) = (page.clone(), show_page.clone());
                        view! {
                            <PanelBody
                                shape
                                calendar
                                times
                                page
                                show_page={move |chosen: usize| show_page.call(chosen)}
                                width={width.clone()}
                                first_weekday
                                step_minutes
                            />
                        }
                    }}
                </Dynamic>
                <List
                    direction=Direction::Horizontal
                    align=Align::Center
                    justify=Justify::SpaceBetween
                    spacing=8.0
                >
                    <Button
                        label=now_label
                        variant=ButtonVariant::Secondary
                        on_click={move || now.call()}
                    />
                    <Show condition=clearable>
                        {move || clone!(clear -> view! {
                            <Button
                                label="Clear"
                                variant=ButtonVariant::Ghost
                                on_click={move || clear.call()}
                            />
                        })}
                    </Show>
                </List>
            </List>
        </Frame>
    }
}

#[component]
fn PanelBody(
    shape: DateTimePanelLayout,
    calendar: DateTimeCalendarHandle,
    times: DateTimeTimesHandle,
    page: ReadSignal<usize>,
    show_page: Callback<usize>,
    width: Memo<f32>,
    first_weekday: Weekday,
    step_minutes: u32,
) -> NodeId {
    let beside = create_memo(clone!(width -> move || {
        (width.get() - PANEL_SPACING - TIME_LIST_WIDTH).min(CALENDAR_MAX_WIDTH)
    }));
    let alone = create_memo(move || width.get().min(CALENDAR_MAX_WIDTH));
    let dates = create_memo(clone!(page -> move || page.get() == 0));
    let times_shown = create_memo(clone!(page -> move || page.get() == 1));
    let (beside_calendar, beside_times) = (calendar.clone(), times.clone());
    let (date_calendar, time_times) = (calendar.clone(), times.clone());
    let alone_date = alone.clone();
    view! {
        <List spacing=PANEL_SPACING>
            <Show condition={shape == DateTimePanelLayout::Beside}>
                {move || clone!(beside beside_calendar beside_times -> view! {
                    <List direction=Direction::Horizontal spacing=PANEL_SPACING>
                        <CalendarPane handle={beside_calendar} width={beside} first_weekday />
                        <TimePane
                            @sizing=ItemSize::Percent(100.0)
                            handle={beside_times}
                            step_minutes
                            max_columns=1
                            height=TIME_LIST_HEIGHT
                        />
                    </List>
                })}
            </Show>
            <Show condition={shape == DateTimePanelLayout::Date}>
                {move || clone!(alone_date date_calendar -> view! {
                    <Centred>
                        <CalendarPane handle={date_calendar} width={alone_date} first_weekday />
                    </Centred>
                })}
            </Show>
            <Show condition={shape == DateTimePanelLayout::Time}>
                {move || clone!(time_times -> view! {
                    <TimePane
                        handle={time_times}
                        step_minutes
                        max_columns=MAX_GRID_COLUMNS
                        height=TIME_GRID_HEIGHT
                    />
                })}
            </Show>
            <Show condition={shape == DateTimePanelLayout::Paged}>
                {move || clone!(dates times_shown page show_page alone calendar times -> view! {
                    <List spacing=PANEL_SPACING>
                        <Tabs
                            options={view! {
                                <ChoiceOption label="Date" />
                                <ChoiceOption label="Time" />
                            }}
                            selected={page}
                            on_change={move |chosen: usize| show_page.call(chosen)}
                        />
                        <Show condition={dates}>
                            {move || clone!(alone calendar -> view! {
                                <Centred>
                                    <CalendarPane handle={calendar} width={alone} first_weekday />
                                </Centred>
                            })}
                        </Show>
                        <Show condition={times_shown}>
                            {move || clone!(times -> view! {
                                <TimePane
                                    handle={times}
                                    step_minutes
                                    max_columns=MAX_GRID_COLUMNS
                                    height=TIME_GRID_HEIGHT
                                />
                            })}
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
fn CalendarPane(handle: DateTimeCalendarHandle, width: Memo<f32>, first_weekday: Weekday) -> NodeId {
    let DateTimeCalendarHandle {
        date,
        shown,
        min,
        max,
        today,
        focused,
        pick,
    } = handle;
    view! {
        <Calendar
            selected={date}
            min
            max
            today
            first_weekday
            focused
            show={shown}
            width
            on_change={move |date| pick.call(date)}
        />
    }
}

#[component]
fn TimePane(handle: DateTimeTimesHandle, step_minutes: u32, max_columns: usize, height: f32) -> NodeId {
    let DateTimeTimesHandle {
        time,
        hour_cycle,
        focused,
        pick,
    } = handle;
    let centred = grid_columns(step_minutes, max_columns) > 1;
    view! {
        <Frame height>
            <unstyled::TimeList
                value={time}
                step_minutes
                hour_cycle
                focused
                row_height={TIME_ROW_HEIGHT}
                max_columns
                spacing={if centred { TIME_GRID_SPACING } else { 0.0 }}
                label="Time"
                scrollbar={scrollbar_style()}
                option={move |handle: TimeOptionHandle| view! {
                    <TimeOptionFace handle centred />
                }}
                on_change={move |time| pick.call(time)}
            />
        </Frame>
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
