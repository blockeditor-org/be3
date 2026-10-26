use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_core::font::Overflow;
use beui_macros::{component, view};

use crate::button::{Button, ButtonVariant};
use crate::calendar::{CALENDAR_WIDTH, Calendar};
use crate::focus_ring::FocusRing;
use crate::popover::{PANEL_PADDING, PopoverPanel};
use crate::scroll::scrollbar_style;
use crate::tabs::Tabs;
use crate::text::{Icon, Text, TextColor};
use crate::theme::{field_border, use_theme};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::datetime::{Date, DateTime, HourCycle, Time, Weekday};
use beui_components_unstyled::{
    ButtonHandle, ChoiceOption, DateSegmentHandle, DateTimeBoxHandle, DateTimeCalendarHandle,
    DateTimePanelHandle, DateTimePanelLayout, DateTimeParts, DateTimeTimesHandle,
    DateTimeTriggerHandle, PopoverPlacement, PopoverTriggerHandle, TimeOptionHandle, grid_columns,
    narrower_than,
};
use beui_core::base::{Align, Direction, Justify, TextAlign};
use beui_core::color::Color32;
use beui_core::icons::{ICON_CALENDAR_DAYS, ICON_CLOCK};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const HEIGHT: f32 = 34.0;
const PADDING_HORIZONTAL: f32 = 8.0;
const SEGMENT_PADDING: f32 = 2.0;
const SEGMENT_RADIUS: f32 = 3.0;
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
    #[prop(default = None)] value: Binding<Option<DateTime>>,
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
                <Frame height=HEIGHT padding={Sides::horizontal(PADDING_HORIZONTAL)}>{field}</Frame>
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
    let border =
        create_memo(move || field_border(&theme, disabled.get(), focused.get(), hovered.get()));
    let fill = create_memo(move || match disabled.get() {
        true => theme.surface.get(),
        false => theme.surface_raised.get(),
    });
    let outer = trigger.is_some();
    let keyboard = focus_ring(focused);
    let ring = create_memo(move || outer && keyboard.get());
    view! {
        <Frame
            border={create_memo(move || ring.get().then_some(Border::FOCUS.offset(FOCUS_RING_OFFSET)))}
            radius={Radius::custom(Radius::Md.px() + 3.0)}
        >
            <Frame
                height=HEIGHT
                fill={ColorRole::custom(fill)}
                border={create_memo(move || Some(Border::new(ColorRole::Custom(border.get()))))}
                radius=Radius::Md
                padding={Sides::new(0.0, 0.0, TRIGGER_GAP, 0.0)}
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
    let fill = create_memo(move || match focused.get() {
        true => theme.accent_soft.get(),
        false => Color32::TRANSPARENT,
    });
    let ink = create_memo(
        move || match (disabled.get(), placeholder.get(), focused.get()) {
            (true, _, _) | (false, true, false) => TextColor::Muted,
            _ => TextColor::Default,
        },
    );
    view! {
        <Frame height={Extent::fraction(1.0)} align={Align2::vertical(Align::Center)}>
            <Frame
                fill={ColorRole::custom(fill)}
                radius={Radius::custom(SEGMENT_RADIUS)}
                padding={Sides::horizontal(SEGMENT_PADDING)}
            >
                <Text color={ink} align=TextAlign::Center>{text}</Text>
            </Frame>
        </Frame>
    }
}

#[component]
fn LiteralFace(text: String) -> NodeId {
    view! {
        <Text color=TextColor::Muted vertical_align=TextAlign::Center>{text}</Text>
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
    let fill =
        create_memo(
            move || match (disabled.get(), active.get() || open.get(), hovered.get()) {
                (true, _, _) => Color32::TRANSPARENT,
                (false, true, _) => theme.pressed.get(),
                (false, false, true) => theme.hover.get(),
                (false, false, false) => Color32::TRANSPARENT,
            },
        );
    let ink = create_memo(move || match disabled.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    let glyph = match parts {
        DateTimeParts::Time => ICON_CLOCK,
        DateTimeParts::Date | DateTimeParts::DateTime => ICON_CALENDAR_DAYS,
    };
    view! {
        <Tooltip label disabled={open}>
            <FocusRing focused offset=1.0>
                <Frame
                    fill={ColorRole::custom(fill)}
                    radius={Radius::custom(SEGMENT_RADIUS)}
                    padding=TRIGGER_PADDING
                >
                    <Text color={ink} align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                </Frame>
            </FocusRing>
        </Tooltip>
    }
}

#[component]
fn PickerPanel(
    handle: DateTimePanelHandle,
    clearable: bool,
    first_weekday: Weekday,
    step_minutes: u32,
) -> NodeId {
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
    let panel_width = create_memo(move || {
        let least = match layout.get() {
            DateTimePanelLayout::Date | DateTimePanelLayout::Paged => CALENDAR_WIDTH,
            DateTimePanelLayout::Time => TIME_GRID_MIN_WIDTH,
            DateTimePanelLayout::Beside => CALENDAR_WIDTH + PANEL_SPACING + TIME_LIST_WIDTH,
        };
        least.max(field_width.get())
    });
    view! {
        <Frame width=panel_width>
            <List spacing=PANEL_SPACING>
                <List direction=Direction::Horizontal spacing=0.0>
                    <Frame width=field_width>{field}</Frame>
                </List>
                <Dynamic value={layout}>|shape: DateTimePanelLayout|
                    <PanelBody
                        shape
                        calendar
                        times
                        page
                        show_page={show_page}
                        width={panel_width}
                        first_weekday
                        step_minutes
                    />
                </Dynamic>
                <List
                    direction=Direction::Horizontal
                    align=Align::Center
                    justify=Justify::SpaceBetween
                    spacing=8.0
                >
                    <Button label=now_label variant=ButtonVariant::Secondary on_click={now} />
                    <Show condition=clearable>
                        <Button label="Clear" variant=ButtonVariant::Ghost on_click={clear} />
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
    let beside = create_memo(move || {
        (width.get() - PANEL_SPACING - TIME_LIST_WIDTH).min(CALENDAR_MAX_WIDTH)
    });
    let alone = create_memo(move || width.get().min(CALENDAR_MAX_WIDTH));
    let dates = create_memo(move || page.get() == 0);
    let times_shown = create_memo(move || page.get() == 1);
    view! {
        <List spacing=PANEL_SPACING>
            <Show condition={shape == DateTimePanelLayout::Beside}>
                <List direction=Direction::Horizontal spacing=PANEL_SPACING>
                    <CalendarPane handle={calendar} width={beside} first_weekday />
                    <TimePane
                        @sizing=ItemSize::Percent(100.0)
                        handle={times}
                        step_minutes
                        max_columns=1
                        height=TIME_LIST_HEIGHT
                    />
                </List>
            </Show>
            <Show condition={shape == DateTimePanelLayout::Date}>
                <Centred>
                    <CalendarPane handle={calendar} width={alone} first_weekday />
                </Centred>
            </Show>
            <Show condition={shape == DateTimePanelLayout::Time}>
                <TimePane
                    handle={times}
                    step_minutes
                    max_columns=MAX_GRID_COLUMNS
                    height=TIME_GRID_HEIGHT
                />
            </Show>
            <Show condition={shape == DateTimePanelLayout::Paged}>
                <List spacing=PANEL_SPACING>
                    <Tabs
                        options={view! {
                            <ChoiceOption label="Date" />
                            <ChoiceOption label="Time" />
                        }}
                        selected={Binding::new(page, show_page)}
                    />
                    <Show condition={dates}>
                        <Centred>
                            <CalendarPane handle={calendar} width={alone} first_weekday />
                        </Centred>
                    </Show>
                    <Show condition={times_shown}>
                        <TimePane
                            handle={times}
                            step_minutes
                            max_columns=MAX_GRID_COLUMNS
                            height=TIME_GRID_HEIGHT
                        />
                    </Show>
                </List>
            </Show>
        </List>
    }
}

#[component]
fn Centred(children: Child) -> NodeId {
    view! {
        <Frame width={Extent::fraction(1.0)} align={Align2::horizontal(Align::Center)}>
            {children}
        </Frame>
    }
}

#[component]
fn CalendarPane(
    handle: DateTimeCalendarHandle,
    width: Memo<f32>,
    first_weekday: Weekday,
) -> NodeId {
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
            selected={Binding::new(date, move |date: Option<Date>| {
                if let Some(date) = date {
                    pick.call(date);
                }
            })}
            min
            max
            today
            first_weekday
            focused
            show={shown}
            width
        />
    }
}

#[component]
fn TimePane(
    handle: DateTimeTimesHandle,
    step_minutes: u32,
    max_columns: usize,
    height: f32,
) -> NodeId {
    let DateTimeTimesHandle {
        time,
        hour_cycle,
        focused,
        pick,
    } = handle;
    let centred = grid_columns(step_minutes, max_columns) > 1;
    view! {
        <Frame height=height>
            <unstyled::TimeList
                value={Binding::new(time, move |time: Option<Time>| {
                    if let Some(time) = time {
                        pick.call(time);
                    }
                })}
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
    let fill = create_memo(
        move || match (selected.get(), active.get(), hovered.get()) {
            (true, _, _) => theme.accent.get(),
            (false, true, _) => theme.pressed.get(),
            (false, false, true) => theme.hover.get(),
            (false, false, false) => Color32::TRANSPARENT,
        },
    );
    let ink = create_memo(move || match selected.get() {
        true => TextColor::OnAccent,
        false => TextColor::Default,
    });
    view! {
        <Frame
            height=TIME_ROW_HEIGHT
            fill={ColorRole::custom(fill)}
            radius=Radius::Md
            border={focus_border(focused, Border::FOCUS.offset(-1.0))}
            padding={Sides::horizontal(10.0)}
        >
            <Text
                overflow=Overflow::Ellipsis
                color={ink}
                align={if centred { TextAlign::Center } else { TextAlign::Start }}
                vertical_align=TextAlign::Center
            >
                {label}
            </Text>
        </Frame>
    }
}
