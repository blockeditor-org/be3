use beui_macros::{component, view};

use crate::action_row::ActionRow;
use crate::border::Separator;
use crate::button::ButtonVariant;
use crate::dock::DockMenu;
use crate::icon_button::{IconButton, IconButtonSize};
use crate::list_row::ListRow;
use crate::sheet::ModalSheet;
use crate::text::{Caption, Heading, Icon};
use crate::theme::{CARD_RADIUS, FOCUS_RING_WIDTH, FONT_BODY, FONT_SMALL, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    DockStackHandle, DockSwitcherCardHandle, DockSwitcherHandle, SHEET_STOPS, TabId,
};
use beui_core::base::{Align, Direction, ItemSize, TextAlign};
use beui_core::color::Color32;
use beui_core::icons::{ICON_ARROW_BACK, ICON_CLOSE, ICON_HOME};
use beui_core::node::NodeId;
use beui_view::reactive::{
    ClickCallback, ForEach, Frame, Grid, List, Memo, Portal, Show, Text, Track, clone,
    create_memo, focus_ring,
};

const BAR_PADDING: f32 = 4.0;
const BAR_SPACING: f32 = 2.0;
const TITLE_PADDING: f32 = 8.0;
const TITLE_SPACING: f32 = 10.0;
const SHEET_PADDING: f32 = 12.0;
const COUNT_SIDE: f32 = 22.0;
const COUNT_TARGET: f32 = 40.0;
const COUNT_RADIUS: u8 = 6;
const COUNT_TARGET_RADIUS: u8 = 8;
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
        away,
        actions,
        menu,
        back,
        others,
        switch_label,
        open_switcher,
    } = handle;
    let theme = use_theme();
    let counted = create_memo(clone!(others -> move || others.get() > 0));
    let pictured = create_memo(clone!(icon -> move || !icon.get().is_empty()));
    view! {
        <List spacing=0.0>
            <Frame
                color={theme.surface.clone()}
                padding_horizontal=BAR_PADDING
                padding_vertical=BAR_PADDING
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                    <Show condition={away}>
                        {move || clone!(back -> view! {
                            <IconButton
                                @test_id={"dock.back"}
                                glyph=ICON_ARROW_BACK
                                label="Back"
                                variant=ButtonVariant::Ghost
                                on_click={move || back.call()}
                            />
                        })}
                    </Show>
                    <Frame @sizing=ItemSize::Percent(100.0) padding_horizontal=TITLE_PADDING>
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=TITLE_SPACING
                        >
                            <Show condition={pictured}>
                                <Icon glyph={icon.clone()} color={theme.accent.clone()} />
                            </Show>
                            <Text
                                @sizing=ItemSize::Percent(100.0)
                                @test_id={"dock.title"}
                                string={title}
                                font_size=FONT_BODY
                                color={theme.text.clone()}
                                ellipsis=true
                            />
                        </List>
                    </Frame>
                    <Portal node={actions} />
                    <DockMenu menu size=IconButtonSize::Regular />
                    <Show condition={counted}>
                        {move || clone!(open_switcher -> view! {
                            <DockTabCount
                                count={others.clone()}
                                label={switch_label.clone()}
                                on_click={move || open_switcher.call()}
                            />
                        })}
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
    let shown = create_memo(clone!(count -> move || count.get().to_string()));
    view! {
        <unstyled::Button
            @test_id={"dock.switch"}
            label
            on_click={move || on_click.call()}
            content={move |handle: unstyled::ButtonHandle| {
                let unstyled::ButtonHandle { hovered, focused, .. } = handle;
                let theme = theme.clone();
                let fill = create_memo(clone!(theme -> move || match hovered.get() {
                    true => theme.hover.get(),
                    false => Color32::TRANSPARENT,
                }));
                let shown = shown.clone();
                view! {
                    <Frame
                        width=COUNT_TARGET
                        height=COUNT_TARGET
                        color={fill}
                        radius=COUNT_TARGET_RADIUS
                        outline={theme.accent.clone()}
                        outline_width=FOCUS_RING_WIDTH
                        outline_visible={focus_ring(focused)}
                        align_horizontal=Align::Center
                        align_vertical=Align::Center
                    >
                        <Frame
                            width=COUNT_SIDE
                            height=COUNT_SIDE
                            radius=COUNT_RADIUS
                            outline={theme.text.clone()}
                            outline_width=COUNT_OUTLINE
                            outline_visible=true
                        >
                            <Text
                                string={shown}
                                font_size=FONT_SMALL
                                color={theme.text.clone()}
                                align=TextAlign::Center
                            />
                        </Frame>
                    </Frame>
                }
            }}
        />
    }
}

#[component]
pub(crate) fn DockSwitcher(handle: DockSwitcherHandle) -> NodeId {
    let DockSwitcherHandle {
        open,
        close,
        tabs,
        card,
        homed,
        home_title,
        home_icon,
        go_home,
    } = handle;
    let none = create_memo(clone!(tabs -> move || tabs.with(Vec::is_empty)));
    let home_glyph = create_memo(move || match home_icon.get() {
        icon if icon.is_empty() => ICON_HOME.to_owned(),
        icon => icon,
    });
    view! {
        <ModalSheet open rest={SHEET_STOPS[2]} on_close={move || close.call()}>
            <List spacing=0.0>
                <Frame padding_horizontal=SHEET_PADDING>
                    <Heading content="Open tabs" />
                </Frame>
                <Frame padding_horizontal=SHEET_PADDING padding_vertical=SHEET_PADDING>
                    <List spacing=CARD_SPACING>
                        <Show condition={none}>
                            <Caption content="Nothing else is open." />
                        </Show>
                        <Grid
                            columns={vec![Track::Fraction(1.0), Track::Fraction(1.0)]}
                            column_spacing=CARD_SPACING
                            row_spacing=CARD_SPACING
                        >
                            <ForEach keys={tabs}>
                                {move |tab: TabId| view! {
                                    <DockSwitcherCard handle={card.call(tab)} />
                                }}
                            </ForEach>
                        </Grid>
                    </List>
                </Frame>
                <Show condition={homed}>
                    {move || clone!(go_home home_title home_glyph -> view! {
                        <Frame padding_horizontal=SHEET_PADDING padding_vertical=SHEET_PADDING>
                            <ActionRow
                                @test_id={"dock.switcher.home"}
                                label={home_title}
                                glyph={home_glyph}
                                on_click={move || go_home.call()}
                            />
                        </Frame>
                    })}
                </Show>
            </List>
        </ModalSheet>
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
    let outline = create_memo(clone!(theme -> move || match current.get() {
        true => theme.accent.get(),
        false => theme.border.get(),
    }));
    let id = tab.value();
    view! {
        <Frame
            height=CARD_HEIGHT
            radius=CARD_RADIUS
            color={theme.surface_raised.clone()}
            outline={outline}
            outline_width=CARD_OUTLINE
            outline_visible=true
            padding_horizontal=CARD_PADDING
            padding_vertical=CARD_PADDING
        >
            <List direction=Direction::Horizontal spacing=0.0>
                <ListRow
                    @sizing=ItemSize::Percent(100.0)
                    @test_id={format!("dock.switcher.tab.{id}")}
                    on_click={move || pick.call()}
                >
                    <Frame padding_vertical=CARD_PADDING>
                        <List spacing=CARD_LINES>
                            <List direction=Direction::Horizontal spacing=0.0>
                                <Icon glyph={icon} color={theme.accent.clone()} />
                            </List>
                            <Text
                                string={title}
                                font_size=FONT_BODY
                                color={theme.text.clone()}
                                ellipsis=true
                            />
                        </List>
                    </Frame>
                </ListRow>
                <Show condition={closable}>
                    {move || clone!(close -> view! {
                        <IconButton
                            @test_id={format!("dock.switcher.close.{id}")}
                            glyph=ICON_CLOSE
                            label="Close tab"
                            variant=ButtonVariant::Ghost
                            on_click={move || close.call()}
                        />
                    })}
                </Show>
            </List>
        </Frame>
    }
}
