use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_macros::{component, view};
use beui_view::types::Overflow;

use crate::button::{Button, ButtonVariant};
use crate::calendar::{CALENDAR_WIDTH, Calendar};
use crate::focus_ring::FocusRing;
use crate::list::List;
use crate::popover::{PANEL_PADDING, popover_surface};
use crate::scroll::scrollbar_style;
use crate::tabs::Tabs;
use crate::text::{Icon, Text, TextColor};
use crate::theme::field_border;
use crate::tokens::{Length, Padding, Size, Space, Stroke, use_size, use_space};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::datetime::{Date, DateTime, HourCycle, Time, Weekday};
use beui_components_unstyled::{
    ButtonHandle, ChoiceOption, DateSegmentHandle, DateTimeBoxHandle, DateTimeCalendarHandle,
    DateTimePanelHandle, DateTimePanelLayout, DateTimeParts, DateTimeTimesHandle,
    DateTimeTriggerHandle, PopoverPlacement, PopupTriggerHandle, TimeOptionHandle, grid_columns,
    narrower_than,
};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::icons::{ICON_CALENDAR_DAYS, ICON_CLOCK};
use beui_view::types::{Align, Direction, Justify, TextAlign};

const HEIGHT: Size = Size::Field;
const PADDING_HORIZONTAL: Space = Space::Md;
const SEGMENT_PADDING: Space = Space::Xs2;
const SEGMENT_RADIUS: Radius = Radius::Sm;
const TRIGGER_PADDING: Space = Space::Xs;
const TRIGGER_GAP: Space = Space::Xs;
const PANEL_SPACING: Space = Space::Lg;
const TIME_LIST_WIDTH: f32 = 128.0;
const TIME_LIST_HEIGHT: f32 = 300.0;
const TIME_GRID_HEIGHT: f32 = 264.0;
const TIME_GRID_MIN_WIDTH: f32 = 280.0;
const TIME_GRID_SPACING: Space = Space::Xs;
const MAX_GRID_COLUMNS: usize = 4;
const CALENDAR_MAX_WIDTH: f32 = 420.0;
const TIME_ROW_HEIGHT: Size = Size::Lg;
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
            placement={PopoverPlacement::Over(PANEL_PADDING.px() as u16)}
            surface={popover_surface(None)}
            segment={|handle: DateSegmentHandle| view! {
                <SegmentFace handle />
            }}
            literal={|text: String| view! {
                <LiteralFace text />
            }}
            segments={|field: Child| view! {
                <Frame height=HEIGHT padding={Padding::horizontal(PADDING_HORIZONTAL)}>
                    {field}
                </Frame>
            }}
            field={|handle: DateTimeBoxHandle| view! {
                <FieldBox handle />
            }}
            trigger={move |handle: DateTimeTriggerHandle| view! {
                <PickerTrigger handle parts />
            }}
            panel={move |handle: DateTimePanelHandle| view! {
                <PickerPanel handle clearable first_weekday step_minutes />
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
    let border = create_memo(move || {
        Some(Border::new(field_border(
            disabled.get(),
            focused.get(),
            hovered.get(),
        )))
    });
    let fill = create_memo(move || match disabled.get() {
        true => ColorRole::Surface,
        false => ColorRole::SurfaceRaised,
    });
    let outer = trigger.is_some();
    let ring = create_memo(move || outer && focused.get());
    view! {
        <FocusRing focused={ring} offset=Stroke::Focus>
            <Frame
                height=HEIGHT
                fill={fill}
                border={border}
                radius=Radius::Md
                padding={Padding::ZERO.right(TRIGGER_GAP)}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::None>
                    {field} @sizing=ItemSize::Percent(100.0)
                    {trigger}
                </List>
            </Frame>
        </FocusRing>
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
    let fill = create_memo(move || match focused.get() {
        true => ColorRole::AccentSoft,
        false => ColorRole::None,
    });
    let ink = create_memo(
        move || match (disabled.get(), placeholder.get(), focused.get()) {
            (true, _, _) | (false, true, false) => TextColor::Muted,
            _ => TextColor::Default,
        },
    );
    view! {
        <Frame height={Extent::fraction(1.0)} align={Align2::vertical(Align::Center)}>
            <Frame fill={fill} radius=SEGMENT_RADIUS padding={Padding::horizontal(SEGMENT_PADDING)}>
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
            PopupTriggerHandle {
                open,
                button:
                    ButtonHandle {
                        hovered,
                        active,
                        focused,
                        disabled,
                        tooltip,
                        ..
                    },
            },
        ..
    } = handle;
    let fill =
        create_memo(
            move || match (disabled.get(), active.get() || open.get(), hovered.get()) {
                (true, _, _) => ColorRole::None,
                (false, true, _) => ColorRole::Pressed,
                (false, false, true) => ColorRole::Hover,
                (false, false, false) => ColorRole::None,
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
        <Tooltip label={tooltip}>
            <FocusRing focused radius=SEGMENT_RADIUS offset=Stroke::Hairline>
                <Frame fill={fill} radius=SEGMENT_RADIUS padding=TRIGGER_PADDING>
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
    let spacing = use_space(PANEL_SPACING);
    let panel_width = create_memo(move || {
        let least = match layout.get() {
            DateTimePanelLayout::Date | DateTimePanelLayout::Paged => CALENDAR_WIDTH,
            DateTimePanelLayout::Time => TIME_GRID_MIN_WIDTH,
            DateTimePanelLayout::Beside => CALENDAR_WIDTH + spacing.get() + TIME_LIST_WIDTH,
        };
        least.max(field_width.get())
    });
    view! {
        <Frame width={create_memo(move || Size::Custom(panel_width.get()))}>
            <List spacing=PANEL_SPACING>
                <List direction=Direction::Horizontal spacing=Space::None>
                    <Frame width={create_memo(move || Size::Custom(field_width.get()))}>
                        {field}
                    </Frame>
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
                    spacing=Space::Md
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
    let spacing = use_space(PANEL_SPACING);
    let beside = create_memo(move || {
        (width.get() - spacing.get() - TIME_LIST_WIDTH).min(CALENDAR_MAX_WIDTH)
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
            width={create_memo(move || Length::Size(Size::Custom(width.get())))}
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
        <Frame height={Size::Custom(height)}>
            <unstyled::TimeList
                value={Binding::new(time, move |time: Option<Time>| {
                    if let Some(time) = time {
                        pick.call(time);
                    }
                })}
                step_minutes
                hour_cycle
                focused
                row_height={use_size(TIME_ROW_HEIGHT).get_untracked()}
                max_columns
                spacing={if centred { use_space(TIME_GRID_SPACING).get_untracked() } else { 0.0 }}
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
    let fill = create_memo(
        move || match (selected.get(), active.get(), hovered.get()) {
            (true, _, _) => ColorRole::Accent,
            (false, true, _) => ColorRole::Pressed,
            (false, false, true) => ColorRole::Hover,
            (false, false, false) => ColorRole::None,
        },
    );
    let ink = create_memo(move || match selected.get() {
        true => TextColor::OnAccent,
        false => TextColor::Default,
    });
    view! {
        <Frame
            height=TIME_ROW_HEIGHT
            fill={fill}
            radius=Radius::Md
            border={focus_border(focused, Border::FOCUS.offset(Stroke::Custom(-Stroke::Hairline.px())))}
            padding={Padding::horizontal(Space::Md)}
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
