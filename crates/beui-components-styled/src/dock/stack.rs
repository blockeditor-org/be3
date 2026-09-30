use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::action_row::ActionRow;
use crate::border::Separator;
use crate::button::ButtonVariant;
use crate::icon_button::IconButton;
use crate::list_row::ListRow;
use crate::scroll::Scroll;
use crate::sheet::{ModalSheet, SHEET_STOPS};
use crate::text::{Caption, Heading, Icon};
use crate::theme::{CARD_RADIUS, FONT_BODY, FONT_SMALL, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{DockStackHandle, TabId};
use beui_core::base::{Align, Direction, ItemSize, TextAlign};
use beui_core::color::Color32;
use beui_core::icons::{ICON_ARROW_BACK, ICON_CLOSE, ICON_HOME};
use beui_core::node::NodeId;
use beui_view::reactive::{
    ClickCallback, Dynamic, ForEach, Frame, Func, List, Memo, Portal, Show, Text, WriteSignal,
    clone, create_memo, create_signal, focus_ring,
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

#[derive(Clone)]
struct Switching {
    shown: Memo<Option<TabId>>,
    tabs: Memo<Vec<TabId>>,
    titles: Func<TabId, String>,
    icons: Func<TabId, String>,
    show: Func<TabId, ()>,
    close: Func<TabId, ()>,
    closable: Func<TabId, bool>,
    set_open: WriteSignal<bool>,
}

#[component]
pub(crate) fn DockStackBar(handle: DockStackHandle, closable: Func<TabId, bool>) -> NodeId {
    let DockStackHandle {
        shown,
        title,
        icon,
        home,
        away,
        tabs,
        actions,
        titles,
        icons,
        back,
        show,
        close,
    } = handle;
    let theme = use_theme();
    let (open, set_open) = create_signal(false);
    let count = create_memo(clone!(tabs -> move || tabs.with(Vec::len)));
    let counted = create_memo(clone!(count -> move || count.get() > 0));
    let pictured = create_memo(clone!(icon -> move || !icon.get().is_empty()));
    let opening = set_open.clone();
    let closing = set_open.clone();
    let switching = Switching {
        shown,
        tabs,
        titles,
        icons,
        show,
        close,
        closable,
        set_open,
    };
    view! {
        <List spacing=0.0>
            <Frame
                color={theme.surface.clone()}
                padding_horizontal=BAR_PADDING
                padding_vertical=BAR_PADDING
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                    <Show condition={away}>
                        <IconButton
                            @test_id={"dock.back"}
                            glyph=ICON_ARROW_BACK
                            label="Back"
                            variant=ButtonVariant::Ghost
                            on_click={move || back.call()}
                        />
                    </Show>
                    <Frame @sizing=ItemSize::Percent(100.0) padding_horizontal=TITLE_PADDING>
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=TITLE_SPACING
                        >
                            <Show condition={pictured}>
                                <Icon glyph={icon} color={theme.accent.clone()} />
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
                    <Show condition={counted}>
                        <DockTabCount count on_click={move || opening.set(true)} />
                    </Show>
                </List>
            </Frame>
            <Separator />
            <ModalSheet open={open} rest={SHEET_STOPS[2]} on_close={move || closing.set(false)}>
                <DockSwitcher switching home />
            </ModalSheet>
        </List>
    }
}

#[component]
fn DockTabCount(count: Memo<usize>, on_click: ClickCallback) -> NodeId {
    let theme = use_theme();
    let label = create_memo(clone!(count -> move || count.get().to_string()));
    let accessibility = create_memo(clone!(count -> move || {
        let mut node = Node::new(Role::Button);
        node.set_label(match count.get() {
            1 => "1 other open tab".to_owned(),
            count => format!("{count} other open tabs"),
        });
        node
    }));
    view! {
        <unstyled::Button
            @test_id={"dock.switch"}
            accessibility
            on_click={move || on_click.call()}
            content={move |handle: unstyled::ButtonHandle| {
                let unstyled::ButtonHandle { hovered, focused, .. } = handle;
                let theme = theme.clone();
                let fill = create_memo(clone!(theme -> move || match hovered.get() {
                    true => theme.hover.get(),
                    false => Color32::TRANSPARENT,
                }));
                let label = label.clone();
                view! {
                    <Frame
                        width=COUNT_TARGET
                        height=COUNT_TARGET
                        color={fill}
                        radius=COUNT_TARGET_RADIUS
                        outline={theme.accent.clone()}
                        outline_width=COUNT_OUTLINE
                        outline_visible={focus_ring(focused)}
                    >
                        <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                            <Frame @sizing=ItemSize::Percent(50.0) />
                            <Frame
                                width=COUNT_SIDE
                                height=COUNT_SIDE
                                radius=COUNT_RADIUS
                                outline={theme.text.clone()}
                                outline_width=COUNT_OUTLINE
                                outline_visible=true
                            >
                                <Text
                                    string={label}
                                    font_size=FONT_SMALL
                                    color={theme.text.clone()}
                                    align=TextAlign::Center
                                />
                            </Frame>
                            <Frame @sizing=ItemSize::Percent(50.0) />
                        </List>
                    </Frame>
                }
            }}
        />
    }
}

#[component]
fn DockSwitcher(switching: Switching, home: Memo<Option<TabId>>) -> NodeId {
    let tabs = switching.tabs.clone();
    let rows = create_memo(clone!(tabs -> move || {
        (0..tabs.get().len().div_ceil(2)).collect::<Vec<usize>>()
    }));
    let none = create_memo(clone!(tabs -> move || tabs.get().is_empty()));
    let homed = create_memo(clone!(home -> move || home.get().is_some()));
    let home_titles = switching.titles.clone();
    let home_title = create_memo(clone!(home -> move || {
        home.get().map(|home| home_titles.call(home)).unwrap_or_default()
    }));
    let home_icons = switching.icons.clone();
    let home_icon = create_memo(clone!(home -> move || {
        home.get()
            .map(|home| home_icons.call(home))
            .filter(|icon| !icon.is_empty())
            .unwrap_or_else(|| ICON_HOME.to_owned())
    }));
    let going = switching.clone();
    view! {
        <List spacing=0.0>
            <Frame padding_horizontal=SHEET_PADDING>
                <Heading content="Open tabs" />
            </Frame>
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <Frame padding_horizontal=SHEET_PADDING padding_vertical=SHEET_PADDING>
                    <List spacing=CARD_SPACING>
                        <Show condition={none}>
                            <Caption content="Nothing else is open." />
                        </Show>
                        <ForEach keys={rows}>
                            {move |row: usize| {
                                let switching = switching.clone();
                                view! {
                                    <DockSwitcherRow switching row />
                                }
                            }}
                        </ForEach>
                    </List>
                </Frame>
            </Scroll>
            <Show condition={homed}>
                <Frame padding_horizontal=SHEET_PADDING padding_vertical=SHEET_PADDING>
                    <ActionRow
                        @test_id={"dock.switcher.home"}
                        label={home_title}
                        glyph={home_icon}
                        on_click={move || {
                            going.set_open.set(false);
                            if let Some(home) = home.get_untracked() {
                                going.show.call(home);
                            }
                        }}
                    />
                </Frame>
            </Show>
        </List>
    }
}

#[component]
fn DockSwitcherRow(switching: Switching, row: usize) -> NodeId {
    let tabs = switching.tabs.clone();
    let left = create_memo(clone!(tabs -> move || tabs.get().get(row * 2).copied()));
    let right = create_memo(move || tabs.get().get(row * 2 + 1).copied());
    let second = switching.clone();
    view! {
        <List direction=Direction::Horizontal spacing=CARD_SPACING>
            <DockSwitcherSlot @sizing=ItemSize::Percent(50.0) switching tab={left} />
            <DockSwitcherSlot @sizing=ItemSize::Percent(50.0) switching={second} tab={right} />
        </List>
    }
}

#[component]
fn DockSwitcherSlot(switching: Switching, tab: Memo<Option<TabId>>) -> NodeId {
    view! {
        <List spacing=0.0>
            <Dynamic value={tab}>
                {move |tab: Option<TabId>| {
                    let switching = switching.clone();
                    match tab {
                        Some(tab) => view! {
                            <DockSwitcherCard switching tab />
                        },
                        None => view! {
                            <Frame height=CARD_HEIGHT />
                        },
                    }
                }}
            </Dynamic>
        </List>
    }
}

#[component]
fn DockSwitcherCard(switching: Switching, tab: TabId) -> NodeId {
    let Switching {
        shown,
        titles,
        icons,
        show,
        close,
        closable,
        set_open,
        ..
    } = switching;
    let theme = use_theme();
    let title = create_memo(move || titles.call(tab));
    let icon = create_memo(move || icons.call(tab));
    let current = create_memo(move || shown.get() == Some(tab));
    let outline = create_memo(clone!(theme -> move || match current.get() {
        true => theme.accent.get(),
        false => theme.border.get(),
    }));
    let closable = closable.call(tab);
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
                    on_click={move || {
                        set_open.set(false);
                        show.call(tab);
                    }}
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
                    <IconButton
                        @test_id={format!("dock.switcher.close.{id}")}
                        glyph=ICON_CLOSE
                        label="Close tab"
                        variant=ButtonVariant::Ghost
                        on_click={move || close.call(tab)}
                    />
                </Show>
            </List>
        </Frame>
    }
}
