use beui_components_styled::frame::{Border, ColorRole, Frame, Radius, focus_border};
use beui_core::font::Overflow;
use beui_macros::{component, view};

use crate::styled::{BarRoom, DockMenu};
use crate::unstyled::{DockStackHandle, DockSwitcherCardHandle, DockSwitcherHandle, TabId};
use beui_components_styled::SHEET_STOPS;
use beui_components_styled::action_row::ActionRow;
use beui_components_styled::border::Separator;
use beui_components_styled::button::Button;
use beui_components_styled::button::ButtonVariant;
use beui_components_styled::list_row::ListRow;
use beui_components_styled::menu_button::MenuButton;
use beui_components_styled::sheet::ModalSheet;
use beui_components_styled::text::{Icon, Text, TextColor, TextSize, TextStyle};
use beui_components_styled::tokens::{Padding, Size, Space, Stroke};
use beui_components_styled::{Grid, List};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::title_action;
use beui_core::base::frame::Align2;
use beui_core::base::{Align, Direction, ItemSize, TextAlign};
use beui_core::icons::{Glyph, ICON_ARROW_LEFT, ICON_CHEVRON_DOWN, ICON_ELLIPSIS_VERTICAL, ICON_X};
use beui_core::node::NodeId;
use beui_view::reactive::*;
use beui_view::types::Track;

const BAR_PADDING: Space = Space::Xs;
const BAR_SPACING: Space = Space::Xs2;
const TITLE_PADDING: Space = Space::Md;
const TITLE_SPACING: Space = Space::Md;
const SHEET_PADDING: Space = Space::Lg;
const COUNT_SIDE: Size = Size::Custom(22.0);
const COUNT_TARGET: Size = Size::Xl;
const COUNT_RADIUS: Radius = Radius::Md;
const COUNT_TARGET_RADIUS: Radius = Radius::Lg;
const COUNT_OUTLINE: Stroke = Stroke::Focus;
const CARD_SPACING: Space = Space::Md;
const CARD_HEIGHT: Size = Size::Custom(96.0);
const CARD_PADDING: Space = Space::Sm;
const CARD_OUTLINE: Stroke = Stroke::Focus;
const CARD_LINES: Space = Space::Sm;

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
        <List spacing=Space::None>
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
                    <List @sizing=ItemSize::Percent(100.0) spacing=Space::None>
                        <Dynamic value={titled}>|titled: bool| {
                            match titled {
                                true => view! {
                                    <DockTitleButton action title icon />
                                },
                                false => view! {
                                    <Frame padding={Padding::horizontal(TITLE_PADDING)}>
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
                true => ColorRole::Hover,
                false => ColorRole::None,
            });
            let opens = create_memo(move || !disabled.get());
            view! {
                <Frame
                    fill={fill}
                    radius=COUNT_TARGET_RADIUS
                    border={focus_border(focused, Border::FOCUS)}
                    height=COUNT_TARGET
                    padding={Padding::horizontal(TITLE_PADDING)}
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
    let shown = create_memo(move || count.get().to_string());
    view! {
        <unstyled::Button
            @test_id={"dock.switch"}
            label
            on_click={on_click}
        >|handle: unstyled::ButtonHandle| {
            let unstyled::ButtonHandle { hovered, focused, .. } = handle;
            let fill = create_memo(move || match hovered.get() {
                true => ColorRole::Hover,
                false => ColorRole::None,
            });
            view! {
                <Frame
                    width=COUNT_TARGET
                    height=COUNT_TARGET
                    fill={fill}
                    radius=COUNT_TARGET_RADIUS
                    border={focus_border(focused, Border::FOCUS)}
                    align=Align::Center
                >
                    <Frame
                        width=COUNT_SIDE
                        height=COUNT_SIDE
                        radius=COUNT_RADIUS
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
            <List spacing=Space::None>
                <Frame padding={Padding::horizontal(SHEET_PADDING)}>
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
                        <List spacing=Space::None>
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
    let outline = create_memo(move || {
        let color = match current.get() {
            true => ColorRole::Accent,
            false => ColorRole::Border,
        };
        Some(Border::new(color).width(CARD_OUTLINE))
    });
    let id = tab.value();
    view! {
        <Frame
            height=CARD_HEIGHT
            radius=Radius::Lg
            fill=ColorRole::SurfaceRaised
            border={outline}
            padding=CARD_PADDING
        >
            <List direction=Direction::Horizontal spacing=Space::None>
                <ListRow
                    @sizing=ItemSize::Percent(100.0)
                    @test_id={format!("dock.switcher.tab.{id}")}
                    on_click={pick}
                >
                    <Frame padding={Padding::vertical(CARD_PADDING)}>
                        <List spacing=CARD_LINES>
                            <List direction=Direction::Horizontal spacing=Space::None>
                                <Text color=TextColor::Accent align=TextAlign::Center>
                                    <Icon glyph={icon} />
                                </Text>
                            </List>
                            <Text overflow=Overflow::Ellipsis>{title}</Text>
                        </List>
                    </Frame>
                </ListRow>
                <List spacing=Space::None>
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
