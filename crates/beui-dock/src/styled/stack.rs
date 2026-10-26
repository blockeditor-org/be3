use beui_components_styled::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_core::font::Overflow;
use beui_macros::{component, view};

use crate::styled::{BarRoom, DockMenu, title_action};
use crate::unstyled::{DockStackHandle, DockSwitcherCardHandle, DockSwitcherHandle, TabId};
use beui_components_styled::action_row::ActionRow;
use beui_components_styled::border::Separator;
use beui_components_styled::button::Button;
use beui_components_styled::button::ButtonVariant;
use beui_components_styled::list_row::ListRow;
use beui_components_styled::menu_button::MenuButton;
use beui_components_styled::sheet::ModalSheet;
use beui_components_styled::text::{Icon, Text, TextColor, TextSize, TextStyle};
use beui_components_styled::theme::use_theme;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::SHEET_STOPS;
use beui_core::base::frame::Align2;
use beui_core::base::{Align, Direction, ItemSize, TextAlign};
use beui_core::color::Color32;
use beui_core::icons::{Glyph, ICON_ARROW_LEFT, ICON_CHEVRON_DOWN, ICON_ELLIPSIS_VERTICAL, ICON_X};
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
        menu,
        back,
        closable_tabs,
        switch_label,
        open_switcher,
    } = handle;
    let action = create_memo(move || menu.with(|nodes| title_action(nodes)));
    let titled = create_memo(move || action.with(Option::is_some));
    view! {
        <List spacing=0.0>
            <Frame fill=ColorRole::Surface padding=BAR_PADDING>
                <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                    <Show condition={backward}>
                        <Button
                            icon_only=true
                            @test_id={"dock.back"}
                            glyph=ICON_ARROW_LEFT
                            label="Back"
                            variant=ButtonVariant::Ghost
                            on_click={back}
                        />
                    </Show>
                    <List @sizing=ItemSize::Percent(100.0) spacing=0.0>
                        <Dynamic value={titled}>|titled: bool| {
                            match titled {
                                true => view! {
                                    <DockTitleButton action title icon />
                                },
                                false => view! {
                                    <Frame padding={Sides::horizontal(TITLE_PADDING)}>
                                        <DockTitle title icon opens=false />
                                    </Frame>
                                },
                            }
                        }</Dynamic>
                    </List>
                    <DockMenu menu room=BarRoom::Phone />
                    <DockTabCount
                        count={closable_tabs}
                        label={switch_label}
                        on_click={open_switcher}
                    />
                </List>
            </Frame>
            <Separator />
        </List>
    }
}

#[component]
fn DockTitleButton(
    action: Memo<Option<Action>>,
    title: Memo<String>,
    icon: Memo<Option<Glyph>>,
) -> NodeId {
    let theme = use_theme();
    let disabled = create_memo(move || {
        action.with(|action| action.as_ref().is_none_or(|action| !action.enabled().get()))
    });
    let label = create_memo(move || {
        action.with(|action| {
            action
                .as_ref()
                .map(|action| action.label().get())
                .unwrap_or_default()
        })
    });
    view! {
        <unstyled::Button
            @test_id={"dock.title"}
            label
            disabled
            on_click={move || {
                if let Some(action) = action.get_untracked() {
                    action.run();
                }
            }}
        >|handle: unstyled::ButtonHandle| {
            let unstyled::ButtonHandle { hovered, focused, .. } = handle;
            let fill = create_memo(move || match hovered.get() && !disabled.get() {
                true => theme.hover.get(),
                false => Color32::TRANSPARENT,
            });
            let opens = create_memo(move || !disabled.get());
            view! {
                <Frame
                    fill={ColorRole::custom(fill)}
                    radius={Radius::custom(COUNT_TARGET_RADIUS)}
                    border={focus_border(focused, Border::FOCUS)}
                    height=COUNT_TARGET
                    padding={Sides::horizontal(TITLE_PADDING)}
                    align={Align2::new(Align::Stretch, Align::Center)}
                >
                    <DockTitle title icon opens />
                </Frame>
            }
        }</unstyled::Button>
    }
}

#[component]
fn DockTitle(title: Memo<String>, icon: Memo<Option<Glyph>>, opens: Prop<bool>) -> NodeId {
    let pictured = create_memo(move || icon.get().is_some());
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=TITLE_SPACING>
            <Show condition={pictured}>
                <Text color=TextColor::Accent align=TextAlign::Center>
                    <Icon glyph={icon} />
                </Text>
            </Show>
            <Text overflow=Overflow::Ellipsis>{title}</Text>
            <Show condition={opens}>
                <Text color=TextColor::Muted align=TextAlign::Center>
                    <Icon glyph=ICON_CHEVRON_DOWN />
                </Text>
            </Show>
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
            let fill = create_memo(move || match hovered.get() {
                true => theme.hover.get(),
                false => Color32::TRANSPARENT,
            });
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
                        <Text size=TextSize::Sm align=TextAlign::Center>{shown}</Text>
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
                    <Text style=TextStyle::HEADING>"Open tabs"</Text>
                </Frame>
                <Frame padding=SHEET_PADDING>
                    <List spacing=CARD_SPACING>
                        <Show condition={none}>
                            <Text style=TextStyle::CAPTION>"Nothing else is open."</Text>
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
        menu,
        pick,
        close,
    } = handle;
    let offered = create_memo(move || menu.with(|items| !items.is_empty()));
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
                            <Text overflow=Overflow::Ellipsis>{title}</Text>
                        </List>
                    </Frame>
                </ListRow>
                <List spacing=0.0>
                    <Show condition={closable}>
                        <Button
                            icon_only=true
                            @test_id={format!("dock.switcher.close.{id}")}
                            glyph=ICON_X
                            label="Close tab"
                            variant=ButtonVariant::Ghost
                            on_click={close}
                        />
                    </Show>
                    <Show condition={offered}>
                        <MenuButton
                            icon_only=true
                            variant=ButtonVariant::Ghost
                            @test_id={format!("dock.switcher.menu.{id}")}
                            label="More"
                            glyph=ICON_ELLIPSIS_VERTICAL
                            items={view! {
                                <unstyled::MenuNodes
                                    nodes={menu}
                                    test_prefix={format!("dock.switcher.menu.{id}")}
                                />
                            }}
                        />
                    </Show>
                </List>
            </List>
        </Frame>
    }
}
