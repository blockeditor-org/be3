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
use beui_components_unstyled::datetime::{Date, DateTime, HourCycle, Time, Weekday};
use beui_components_unstyled::{
    ChoiceOption, DateSegmentHandle, DateTimeBoxHandle, DateTimePanelHandle, DateTimeParts,
    DateTimeTriggerHandle, PopoverPlacement, PopoverTriggerHandle, TimeOptionHandle,
    narrower_than,
};
use beui_core::base::{Align, Direction, Justify, TextAlign};
use beui_core::color::Color32;
use beui_core::icons::{ICON_CALENDAR_MONTH, ICON_SCHEDULE};
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, Child, Dynamic, Frame, ItemSize, List, Memo, Prop, ReadSignal, Show, Text,
    WriteSignal, clone, create_memo, focus_ring,
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
    let boxed = view! {
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
                <Show condition={trigger.is_some()}>
                    {trigger.unwrap_or_else(|| unreachable!())}
                </Show>
            </List>
        </Frame>
    };
    match trigger.is_some() {
        false => boxed,
        true => view! {
            <Frame
                outline={theme.accent.clone()}
                outline_width=FOCUS_RING_WIDTH
                radius={RADIUS + 3}
                outline_offset=FOCUS_RING_OFFSET
                outline_visible={focus_ring(focused)}
            >
                {boxed}
            </Frame>
        },
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
                hovered,
                active,
                focused,
                disabled,
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
fn PickerPanel(
    handle: DateTimePanelHandle,
    clearable: bool,
    first_weekday: Weekday,
    step_minutes: u32,
) -> NodeId {
    let DateTimePanelHandle {
        field,
        field_width,
        parts,
        hour_cycle,
        date,
        time,
        shown,
        min,
        max,
        today,
        calendar_focused,
        list_focused,
        tab,
        set_tab,
        pick_date,
        pick_time,
        now,
        now_label,
        clear,
    } = handle;
    let narrow = narrower_than(STACK_BREAKPOINT);
    let layout = create_memo(clone!(narrow -> move || match parts {
        DateTimeParts::Date => PanelLayout::Date,
        DateTimeParts::Time => PanelLayout::Time,
        DateTimeParts::DateTime if narrow.get() => PanelLayout::Paged,
        DateTimeParts::DateTime => PanelLayout::Beside,
    }));
    let panel_width = create_memo(clone!(layout field_width -> move || {
        let least = match layout.get() {
            PanelLayout::Date | PanelLayout::Paged => CALENDAR_WIDTH,
            PanelLayout::Time => TIME_GRID_MIN_WIDTH,
            PanelLayout::Beside => CALENDAR_WIDTH + PANEL_SPACING + TIME_LIST_WIDTH,
        };
        least.max(field_width.get())
    }));
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
                    <Frame width={field_width}>{field}</Frame>
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
                    <Button
                        label=now_label
                        variant=ButtonVariant::Secondary
                        on_click={move || now.call()}
                    />
                    <Show condition=clearable>
                        <Button
                            label="Clear"
                            variant=ButtonVariant::Ghost
                            on_click={move || clear.call()}
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
                <List direction=Direction::Horizontal spacing=PANEL_SPACING>
                    <CalendarPane pieces={beside_pieces.clone()} width={beside} />
                    <TimePane
                        @sizing=ItemSize::Percent(100.0)
                        pieces={beside_pieces}
                        columns=1
                        height=TIME_LIST_HEIGHT
                    />
                </List>
            </Show>
            <Show condition={shape == PanelLayout::Date}>
                <Centred>
                    <CalendarPane pieces={date_pieces} width={alone} />
                </Centred>
            </Show>
            <Show condition={shape == PanelLayout::Time}>
                <TimePane pieces={time_pieces} columns height=TIME_GRID_HEIGHT />
            </Show>
            <Show condition={shape == PanelLayout::Paged}>
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
                        <Centred>
                            <CalendarPane pieces={tab_dates} width={tab_alone} />
                        </Centred>
                    </Show>
                    <Show condition={times}>
                        <TimePane pieces={tab_times} columns height=TIME_GRID_HEIGHT />
                    </Show>
                </List>
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
