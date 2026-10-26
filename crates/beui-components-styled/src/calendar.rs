use crate::frame::{Border, ColorRole, Frame, Radius};
use accesskit::Node;
use beui_macros::{component, view};

use crate::button::{Button, ButtonSize};
use crate::button::{ButtonFace, ButtonVariant};
use crate::focus_ring::FocusRing;
use crate::text::{Text, TextColor, TextSize};
use crate::list::List;
use crate::tokens::{Length, Size, Space, Stroke, use_space};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::datetime::{Date, Weekday};
use beui_components_unstyled::{CalendarCellHandle, CalendarHeaderHandle, CalendarMode};
use beui_core::base::{Align, Direction, Justify, TextAlign};
use beui_core::icons::{ICON_CHEVRON_DOWN, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_CHEVRON_UP};
use beui_core::node::NodeId;
use beui_view::reactive::*;

pub const CALENDAR_WIDTH: f32 = 7.0 * DAY_SIZE + 6.0 * SPACING.px();
const DAY_SIZE: f32 = 38.0;
const MONTH_HEIGHT: f32 = 56.0;
const YEAR_HEIGHT: f32 = 44.0;
const WEEKDAY_HEIGHT: Size = Size::Md;
const SPACING: Space = Space::Xs2;

#[component]
pub fn Calendar(
    #[prop(default = None)] selected: Binding<Option<Date>>,
    #[prop(default = None)] min: Prop<Option<Date>>,
    #[prop(default = None)] max: Prop<Option<Date>>,
    #[prop(default = None)] today: Prop<Option<Date>>,
    #[prop(default = Weekday::Monday)] first_weekday: Weekday,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = None)] show: Prop<Option<Date>>,
    #[prop(default = Length::Size(Size::Custom(CALENDAR_WIDTH)))] width: Prop<Length>,
) -> NodeId {
    view! {
        <Frame width=width>
            <unstyled::Calendar
                selected
                min
                max
                today
                first_weekday
                focused
                show
                spacing={use_space(SPACING).get_untracked()}
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
    let month_shown = create_memo(move || !years_open.get());
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
            <Button
                icon_only=true
                glyph=ICON_CHEVRON_LEFT
                label={previous_label}
                size=ButtonSize::Small
                variant=ButtonVariant::Ghost
                disabled={no_previous}
                on_click={move || previous.call(())}
            />
            <List
                @sizing=ItemSize::Percent(100.0)
                direction=Direction::Horizontal
                justify=Justify::Center
                spacing=Space::None
            >
                <Show condition={month_shown}>
                    <HeaderToggle
                        label={month_label}
                        accessibility={month_toggle}
                        open={months_open}
                        on_click={move || show_months.call(())}
                    />
                </Show>
                <HeaderToggle
                    label={year_label}
                    accessibility={year_toggle}
                    open={years_open}
                    on_click={move || show_years.call(())}
                />
            </List>
            <Button
                icon_only=true
                glyph=ICON_CHEVRON_RIGHT
                label={next_label}
                size=ButtonSize::Small
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
        true => ICON_CHEVRON_UP,
        false => ICON_CHEVRON_DOWN,
    });
    view! {
        <unstyled::Button label accessibility on_click={on_click}>|button: unstyled::ButtonHandle|
            <ButtonFace handle={button} variant=ButtonVariant::Ghost trailing_glyph={glyph} />
        </unstyled::Button>
    }
}

#[component]
fn WeekdayLabel(weekday: Weekday) -> NodeId {
    view! {
        <Frame height=WEEKDAY_HEIGHT>
            <Text
                size=TextSize::Sm
                color=TextColor::Muted
                align=TextAlign::Center
                vertical_align=TextAlign::Center
            >
                {weekday.narrow_name().to_owned()}
            </Text>
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
        CalendarMode::Days => Size::Custom(DAY_SIZE),
        CalendarMode::Months => Size::Custom(MONTH_HEIGHT),
        CalendarMode::Years => Size::Custom(YEAR_HEIGHT),
    };
    let fill = create_memo(move || {
        cell_fill(
            selected.get(),
            disabled.get(),
            hovered.get(),
            active.get(),
        )
    });
    let ink = create_memo(move || cell_ink(selected.get(), disabled.get(), outside.get()));
    let ring = create_memo(move || marked.get() && !selected.get());
    view! {
        <FocusRing focused radius=Radius::Md offset=Stroke::Hairline>
            <Frame
                height=height
                fill={fill}
                border={create_memo(move || ring.get().then_some(Border::new(ColorRole::Accent)))}
                radius=Radius::Md
            >
                <Text color={ink} align=TextAlign::Center vertical_align=TextAlign::Center>
                    {label}
                </Text>
            </Frame>
        </FocusRing>
    }
}

fn cell_fill(
    selected: bool,
    disabled: bool,
    hovered: bool,
    active: bool,
) -> ColorRole {
    match (selected, disabled, active, hovered) {
        (true, true, _, _) => ColorRole::Track,
        (true, false, _, _) => ColorRole::Accent,
        (false, true, _, _) => ColorRole::None,
        (false, false, true, _) => ColorRole::Pressed,
        (false, false, false, true) => ColorRole::Hover,
        (false, false, false, false) => ColorRole::None,
    }
}

fn cell_ink(selected: bool, disabled: bool, quiet: bool) -> TextColor {
    match (selected, disabled, quiet) {
        (true, _, _) => TextColor::OnAccent,
        (false, true, _) => TextColor::Faint,
        (false, false, true) => TextColor::Muted,
        (false, false, false) => TextColor::Default,
    }
}
