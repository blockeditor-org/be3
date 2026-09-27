use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::base::{Align, Direction, TextAlign};
use crate::color::Color32;
use crate::datetime::{Date, Weekday};
use crate::icons::{
    ICON_ARROW_DROP_DOWN, ICON_ARROW_DROP_UP, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT,
};
use crate::node::NodeId;
use crate::reactive::{
    Callback, ClickCallback, Frame, ItemSize, List, Memo, Prop, ReadSignal, Show, Text, clone,
    create_memo, focus_ring,
};
use crate::styled::button::{ButtonFace, ButtonVariant};
use crate::styled::icon_button::{IconButton, IconButtonSize};
use crate::styled::theme::{FONT_BODY, FONT_SMALL, RADIUS, ThemeStore, use_theme};
use crate::unstyled;
use crate::unstyled::{
    CalendarDayHandle, CalendarHeaderHandle, CalendarMode, CalendarMonthHandle, CalendarYearHandle,
};

pub(crate) const CALENDAR_WIDTH: f32 = 7.0 * DAY_SIZE + 6.0 * SPACING;
const DAY_SIZE: f32 = 38.0;
const MONTH_HEIGHT: f32 = 56.0;
const YEAR_HEIGHT: f32 = 44.0;
const WEEKDAY_HEIGHT: f32 = 24.0;
const SPACING: f32 = 2.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 1.0;
const TODAY_RING_WIDTH: f32 = 1.0;

#[component]
pub fn Calendar(
    selected: Prop<Option<Date>>,
    #[prop(default = None)] min: Prop<Option<Date>>,
    #[prop(default = None)] max: Prop<Option<Date>>,
    #[prop(default = None)] today: Prop<Option<Date>>,
    #[prop(default = Weekday::Monday)] first_weekday: Weekday,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = None)] show: Prop<Option<Date>>,
    on_change: Callback<Date>,
) -> NodeId {
    view! {
        <Frame width=CALENDAR_WIDTH>
            <unstyled::Calendar
                selected
                min
                max
                today
                first_weekday
                focused
                show
                spacing=SPACING
                on_change={move |date| on_change.call(date)}
                header={|handle: CalendarHeaderHandle| view! {
                    <CalendarHeader handle />
                }}
                weekday={|weekday: Weekday| view! {
                    <WeekdayLabel weekday />
                }}
                day={|handle: CalendarDayHandle| view! {
                    <DayFace handle />
                }}
                month={|handle: CalendarMonthHandle| view! {
                    <MonthFace handle />
                }}
                year={|handle: CalendarYearHandle| view! {
                    <YearFace handle />
                }}
            />
        </Frame>
    }
}

#[component]
fn CalendarHeader(handle: CalendarHeaderHandle) -> NodeId {
    let CalendarHeaderHandle {
        month_label,
        year_label,
        mode,
        previous,
        next,
        show_months,
        show_years,
        can_previous,
        can_next,
        ..
    } = handle;
    let unit = create_memo(clone!(mode -> move || match mode.get() {
        CalendarMode::Days => "month",
        CalendarMode::Months => "year",
        CalendarMode::Years => "years",
    }));
    let previous_label = create_memo(clone!(unit -> move || format!("Previous {}", unit.get())));
    let next_label = create_memo(move || format!("Next {}", unit.get()));
    let no_previous = create_memo(move || !can_previous.get());
    let no_next = create_memo(move || !can_next.get());
    let months_open = create_memo(clone!(mode -> move || mode.get() == CalendarMode::Months));
    let years_open = create_memo(clone!(mode -> move || mode.get() == CalendarMode::Years));
    let month_shown = create_memo(clone!(years_open -> move || !years_open.get()));
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
            <IconButton
                glyph={ICON_CHEVRON_LEFT.to_owned()}
                label={previous_label}
                size=IconButtonSize::Compact
                variant=ButtonVariant::Ghost
                disabled={no_previous}
                on_click={move || previous.call(())}
            />
            <List @sizing=ItemSize::Percent(100.0) direction=Direction::Horizontal spacing=0.0>
                <List @sizing=ItemSize::Percent(100.0) spacing=0.0 />
                <Show condition={month_shown}>
                    <HeaderToggle
                        label={month_label}
                        choose="choose a month"
                        open={months_open}
                        on_click={move || show_months.call(())}
                    />
                </Show>
                <HeaderToggle
                    label={year_label}
                    choose="choose a year"
                    open={years_open}
                    on_click={move || show_years.call(())}
                />
                <List @sizing=ItemSize::Percent(100.0) spacing=0.0 />
            </List>
            <IconButton
                glyph={ICON_CHEVRON_RIGHT.to_owned()}
                label={next_label}
                size=IconButtonSize::Compact
                variant=ButtonVariant::Ghost
                disabled={no_next}
                on_click={move || next.call(())}
            />
        </List>
    }
}

#[component]
fn HeaderToggle(
    label: Memo<String>,
    choose: &'static str,
    open: Memo<bool>,
    on_click: ClickCallback,
) -> NodeId {
    let accessibility = create_memo(clone!(label open -> move || {
        let mut node = Node::new(Role::Button);
        node.set_label(match open.get() {
            true => format!("{}, back to the days", label.get()),
            false => format!("{}, {choose}", label.get()),
        });
        node.set_expanded(open.get());
        node
    }));
    let glyph = create_memo(move || match open.get() {
        true => ICON_ARROW_DROP_UP.to_owned(),
        false => ICON_ARROW_DROP_DOWN.to_owned(),
    });
    view! {
        <unstyled::Button
            accessibility
            on_click={move || on_click.call()}
            content={move |button: unstyled::ButtonHandle| view! {
                <ButtonFace
                    handle={button}
                    variant=ButtonVariant::Ghost
                    label={label.clone()}
                    glyph=String::new()
                    trailing_glyph={glyph.clone()}
                    disabled=false
                />
            }}
        />
    }
}

#[component]
fn WeekdayLabel(weekday: Weekday) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame height=WEEKDAY_HEIGHT>
            <Text
                string={weekday.narrow_name().to_owned()}
                font_size=FONT_SMALL
                color={theme.text_muted.clone()}
                align=TextAlign::Center
                vertical_align=TextAlign::Center
            />
        </Frame>
    }
}

#[component]
fn DayFace(handle: CalendarDayHandle) -> NodeId {
    let CalendarDayHandle {
        date,
        selected,
        today,
        outside,
        disabled,
        hovered,
        active,
        focused,
    } = handle;
    let number = create_memo(move || date.get().day.to_string());
    view! {
        <CellFace
            text={number}
            height=DAY_SIZE
            selected
            marked={today}
            quiet={outside}
            disabled
            hovered
            active
            focused
        />
    }
}

#[component]
fn MonthFace(handle: CalendarMonthHandle) -> NodeId {
    let CalendarMonthHandle {
        month,
        selected,
        current,
        disabled,
        hovered,
        active,
        focused,
    } = handle;
    let name = create_memo(move || month.get().month_name()[..3].to_owned());
    let quiet = create_memo(|| false);
    view! {
        <CellFace
            text={name}
            height=MONTH_HEIGHT
            selected
            marked={current}
            quiet
            disabled
            hovered
            active
            focused
        />
    }
}

#[component]
fn YearFace(handle: CalendarYearHandle) -> NodeId {
    let CalendarYearHandle {
        year,
        selected,
        current,
        disabled,
        hovered,
        active,
        focused,
    } = handle;
    let name = create_memo(move || year.get().to_string());
    let quiet = create_memo(|| false);
    view! {
        <CellFace
            text={name}
            height=YEAR_HEIGHT
            selected
            marked={current}
            quiet
            disabled
            hovered
            active
            focused
        />
    }
}

#[component]
fn CellFace(
    text: Memo<String>,
    height: f32,
    selected: Memo<bool>,
    marked: Memo<bool>,
    quiet: Memo<bool>,
    disabled: Memo<bool>,
    hovered: ReadSignal<bool>,
    active: ReadSignal<bool>,
    focused: ReadSignal<bool>,
) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(clone!(theme selected disabled -> move || {
        cell_fill(&theme, selected.get(), disabled.get(), hovered.get(), active.get())
    }));
    let ink = create_memo(clone!(theme selected disabled -> move || {
        cell_ink(&theme, selected.get(), disabled.get(), quiet.get())
    }));
    let ring = create_memo(clone!(selected -> move || marked.get() && !selected.get()));
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius={RADIUS + 2}
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={focus_ring(focused)}
        >
            <Frame
                height
                color={fill}
                outline={theme.accent.clone()}
                outline_width=TODAY_RING_WIDTH
                outline_visible={ring}
                radius=RADIUS
            >
                <Text
                    string={text}
                    font_size=FONT_BODY
                    color={ink}
                    align=TextAlign::Center
                    vertical_align=TextAlign::Center
                />
            </Frame>
        </Frame>
    }
}

fn cell_fill(
    theme: &ThemeStore,
    selected: bool,
    disabled: bool,
    hovered: bool,
    active: bool,
) -> Color32 {
    match (selected, disabled, active, hovered) {
        (true, true, _, _) => theme.track.get(),
        (true, false, _, _) => theme.accent.get(),
        (false, true, _, _) => Color32::TRANSPARENT,
        (false, false, true, _) => theme.pressed.get(),
        (false, false, false, true) => theme.hover.get(),
        (false, false, false, false) => Color32::TRANSPARENT,
    }
}

fn cell_ink(theme: &ThemeStore, selected: bool, disabled: bool, quiet: bool) -> Color32 {
    match (selected, disabled, quiet) {
        (true, _, _) => theme.on_accent.get(),
        (false, true, _) => theme.border.get(),
        (false, false, true) => theme.text_muted.get(),
        (false, false, false) => theme.text.get(),
    }
}
