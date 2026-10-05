use accesskit::Node;
use beui_macros::{component, view};

use crate::button::{ButtonFace, ButtonVariant};
use crate::focus_ring::FocusRing;
use crate::icon_button::{IconButton, IconButtonSize};
use crate::theme::{FONT_BODY, FONT_SMALL, RADIUS, ThemeStore, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::datetime::{Date, Weekday};
use beui_components_unstyled::{CalendarCellHandle, CalendarHeaderHandle, CalendarMode};
use beui_core::base::{Align, Direction, Justify, TextAlign};
use beui_core::color::Color32;
use beui_core::icons::{
    ICON_ARROW_DROP_DOWN, ICON_ARROW_DROP_UP, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT,
};
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, ClickCallback, Frame, ItemSize, List, Memo, Prop, Show, Text, clone, create_memo,
};

pub const CALENDAR_WIDTH: f32 = 7.0 * DAY_SIZE + 6.0 * SPACING;
const DAY_SIZE: f32 = 38.0;
const MONTH_HEIGHT: f32 = 56.0;
const YEAR_HEIGHT: f32 = 44.0;
const WEEKDAY_HEIGHT: f32 = 24.0;
const SPACING: f32 = 2.0;
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
    #[prop(default = CALENDAR_WIDTH)] width: Prop<f32>,
    on_change: Callback<Date>,
) -> NodeId {
    view! {
        <Frame width>
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
                cell={|handle: CalendarCellHandle| view! {
                    <CellFace handle />
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
        previous,
        next,
        show_months,
        show_years,
        can_previous,
        can_next,
        previous_label,
        next_label,
        months_open,
        years_open,
        month_toggle,
        year_toggle,
        ..
    } = handle;
    let no_previous = create_memo(move || !can_previous.get());
    let no_next = create_memo(move || !can_next.get());
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
            <List
                @sizing=ItemSize::Percent(100.0)
                direction=Direction::Horizontal
                justify=Justify::Center
                spacing=0.0
            >
                <Show condition={month_shown}>
                    {move || clone!(month_label months_open month_toggle show_months -> view! {
                        <HeaderToggle
                            label={month_label}
                            accessibility={month_toggle}
                            open={months_open}
                            on_click={move || show_months.call(())}
                        />
                    })}
                </Show>
                <HeaderToggle
                    label={year_label}
                    accessibility={year_toggle}
                    open={years_open}
                    on_click={move || show_years.call(())}
                />
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
    accessibility: Memo<Node>,
    open: Memo<bool>,
    on_click: ClickCallback,
) -> NodeId {
    let glyph = create_memo(move || match open.get() {
        true => ICON_ARROW_DROP_UP.to_owned(),
        false => ICON_ARROW_DROP_DOWN.to_owned(),
    });
    view! {
        <unstyled::Button
            label
            accessibility
            on_click={move || on_click.call()}
            content={move |button: unstyled::ButtonHandle| view! {
                <ButtonFace
                    handle={button}
                    variant=ButtonVariant::Ghost
                    trailing_glyph={glyph.clone()}
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
fn CellFace(handle: CalendarCellHandle) -> NodeId {
    let CalendarCellHandle {
        kind,
        label,
        selected,
        marked,
        outside,
        disabled,
        hovered,
        active,
        focused,
    } = handle;
    let height = match kind {
        CalendarMode::Days => DAY_SIZE,
        CalendarMode::Months => MONTH_HEIGHT,
        CalendarMode::Years => YEAR_HEIGHT,
    };
    let theme = use_theme();
    let fill = create_memo(clone!(theme selected disabled -> move || {
        cell_fill(&theme, selected.get(), disabled.get(), hovered.get(), active.get())
    }));
    let ink = create_memo(clone!(theme selected disabled -> move || {
        cell_ink(&theme, selected.get(), disabled.get(), outside.get())
    }));
    let ring = create_memo(clone!(selected -> move || marked.get() && !selected.get()));
    view! {
        <FocusRing focused radius={RADIUS + 2} offset=FOCUS_RING_OFFSET>
            <Frame
                height
                color={fill}
                outline={theme.accent.clone()}
                outline_width=TODAY_RING_WIDTH
                outline_visible={ring}
                radius=RADIUS
            >
                <Text
                    string={label}
                    font_size=FONT_BODY
                    color={ink}
                    align=TextAlign::Center
                    vertical_align=TextAlign::Center
                />
            </Frame>
        </FocusRing>
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
