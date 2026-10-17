use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_macros::{component, view};

use crate::action_row::ActionRow;
use crate::border::Separator;
use crate::button::ButtonVariant;
use crate::dock::DockMenu;
use crate::icon_button::{IconButton, IconButtonSize};
use crate::list_row::ListRow;
use crate::sheet::ModalSheet;
use crate::text::{Icon, Text, TextColor, TextSize, TextStyle};
use crate::theme::use_theme;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    DockStackHandle, DockSwitcherCardHandle, DockSwitcherHandle, SHEET_STOPS, TabId,
};
use beui_core::base::{Align, Direction, ItemSize, TextAlign};
use beui_core::color::Color32;
use beui_core::icons::{ICON_ARROW_BACK, ICON_CLOSE};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const BAR_PADDING: f32 = 4.0;
const BAR_SPACING: f32 = 2.0;
const TITLE_PADDING: f32 = 8.0;
const TITLE_SPACING: f32 = 10.0;
const SHEET_PADDING: f32 = 12.0;
const COUNT_SIDE: f32 = 22.0;
const COUNT_TARGET: f32 = 40.0;
const COUNT_RADIUS: f32 = 6.0;
const COUNT_TARGET_RADIUS: f32 = 8.0;
const COUNT_OUTLINE: f32 = 2.0;
const CARD_SPACING: f32 = 10.0;
const CARD_HEIGHT: f32 = 96.0;
const CARD_PADDING: f32 = 6.0;
const CARD_OUTLINE: f32 = 1.5;
const CARD_LINES: f32 = 6.0;

#[component]
pub(crate) fn DockStackBar(handle: DockStackHandle) -> NodeId {
    let DockStackHandle {
        title,
        icon,
        backward,
        actions,
        menu,
        back,
        others,
        switchable,
        switch_label,
        open_switcher,
    } = handle;
    let pictured = create_memo(move || !icon.get().is_empty());
    view! {
        <List spacing=0.0>
            <Frame fill=ColorRole::Surface padding=BAR_PADDING>
                <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                    <Show condition={backward}>
                        <IconButton
                            @test_id={"dock.back"}
                            glyph=ICON_ARROW_BACK
                            label="Back"
                            variant=ButtonVariant::Ghost
                            on_click={back}
                        />
                    </Show>
                    <Frame
                        @sizing=ItemSize::Percent(100.0)
                        padding={Sides::horizontal(TITLE_PADDING)}
                    >
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=TITLE_SPACING
                        >
                            <Show condition={pictured}>
                                <Text color=TextColor::Accent align=TextAlign::Center>
                                    <Icon glyph={icon} />
                                </Text>
                            </Show>
                            <Text
                                @sizing=ItemSize::Percent(100.0)
                                @test_id={"dock.title"}
                                string={title}
                                ellipsis=true
                            />
                        </List>
                    </Frame>
                    <Portal node={actions} />
                    <DockMenu menu size=IconButtonSize::Regular />
                    <Show condition={switchable}>
                        <DockTabCount
                            count={others}
                            label={switch_label}
                            on_click={open_switcher}
                        />
                    </Show>
                </List>
            </Frame>
            <Separator />
        </List>
    }
}

#[component]
fn DockTabCount(count: Memo<usize>, label: Memo<String>, on_click: ClickCallback) -> NodeId {
    let theme = use_theme();
    let shown = create_memo(move || count.get().to_string());
    view! {
        <unstyled::Button
            @test_id={"dock.switch"}
            label
            on_click={on_click}
        >|handle: unstyled::ButtonHandle| {
            let unstyled::ButtonHandle { hovered, focused, .. } = handle;
            let theme = theme;
            let fill = create_memo(move || match hovered.get() {
                true => theme.hover.get(),
                false => Color32::TRANSPARENT,
            });
            let shown = shown;
            view! {
                <Frame
                    width=COUNT_TARGET
                    height=COUNT_TARGET
                    fill={ColorRole::custom(fill)}
                    radius={Radius::custom(COUNT_TARGET_RADIUS)}
                    border={focus_border(focused, Border::FOCUS)}
                    align=Align::Center
                >
                    <Frame
                        width=COUNT_SIDE
                        height=COUNT_SIDE
                        radius={Radius::custom(COUNT_RADIUS)}
                        border={Border::new(ColorRole::Text).width(COUNT_OUTLINE)}
                    >
                        <Text string={shown} size=TextSize::Sm align=TextAlign::Center />
                    </Frame>
                </Frame>
            }
        }</unstyled::Button>
    }
}

#[component]
pub(crate) fn DockSwitcher(handle: DockSwitcherHandle) -> NodeId {
    let DockSwitcherHandle {
        open,
        close,
        tabs,
        fixed,
        card,
    } = handle;
    let none = create_memo(move || tabs.with(Vec::is_empty));
    let rowed = create_memo(move || fixed.with(|fixed| !fixed.is_empty()));
    view! {
        <ModalSheet open rest={SHEET_STOPS[2]} on_close={close}>
            <List spacing=0.0>
                <Frame padding={Sides::horizontal(SHEET_PADDING)}>
                    <Text style=TextStyle::HEADING string="Open tabs" />
                </Frame>
                <Frame padding=SHEET_PADDING>
                    <List spacing=CARD_SPACING>
                        <Show condition={none}>
                            <Text style=TextStyle::CAPTION string="Nothing else is open." />
                        </Show>
                        <Grid
                            columns={vec![Track::Fraction(1.0), Track::Fraction(1.0)]}
                            column_spacing=CARD_SPACING
                            row_spacing=CARD_SPACING
                        >
                            <ForEach keys={tabs}>|tab: TabId|
                                <DockSwitcherCard handle={card.call(tab)} />
                            </ForEach>
                        </Grid>
                    </List>
                </Frame>
                <Show condition={rowed}>
                    <Frame padding=SHEET_PADDING>
                        <List spacing=0.0>
                            <ForEach keys={fixed}>|tab: TabId|
                                <DockSwitcherRow handle={card.call(tab)} />
                            </ForEach>
                        </List>
                    </Frame>
                </Show>
            </List>
        </ModalSheet>
    }
}

#[component]
fn DockSwitcherRow(handle: DockSwitcherCardHandle) -> NodeId {
    let DockSwitcherCardHandle {
        tab,
        title,
        icon,
        pick,
        ..
    } = handle;
    view! {
        <ActionRow
            @test_id={format!("dock.switcher.tab.{}", tab.value())}
            label={title}
            glyph={icon}
            on_click={pick}
        />
    }
}

#[component]
fn DockSwitcherCard(handle: DockSwitcherCardHandle) -> NodeId {
    let DockSwitcherCardHandle {
        tab,
        title,
        icon,
        current,
        closable,
        pick,
        close,
    } = handle;
    let theme = use_theme();
    let outline = create_memo(move || match current.get() {
        true => theme.accent.get(),
        false => theme.border.get(),
    });
    let id = tab.value();
    view! {
        <Frame
            height=CARD_HEIGHT
            radius=Radius::Lg
            fill=ColorRole::SurfaceRaised
            border={create_memo(move || Some(Border::new(ColorRole::Custom(outline.get())).width(CARD_OUTLINE)))}
            padding=CARD_PADDING
        >
            <List direction=Direction::Horizontal spacing=0.0>
                <ListRow
                    @sizing=ItemSize::Percent(100.0)
                    @test_id={format!("dock.switcher.tab.{id}")}
                    on_click={pick}
                >
                    <Frame padding={Sides::vertical(CARD_PADDING)}>
                        <List spacing=CARD_LINES>
                            <List direction=Direction::Horizontal spacing=0.0>
                                <Text color=TextColor::Accent align=TextAlign::Center>
                                    <Icon glyph={icon} />
                                </Text>
                            </List>
                            <Text string={title} ellipsis=true />
                        </List>
                    </Frame>
                </ListRow>
                <Show condition={closable}>
                    <IconButton
                        @test_id={format!("dock.switcher.close.{id}")}
                        glyph=ICON_CLOSE
                        label="Close tab"
                        variant=ButtonVariant::Ghost
                        on_click={close}
                    />
                </Show>
            </List>
        </Frame>
    }
}
