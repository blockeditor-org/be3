use beui_macros::{component, view};

use crate::action_row::ActionRowFace;
use crate::border::Separator;
use crate::sheet::ModalSheet;
use crate::text::{Icon, Text, TextColor, TextSize};
use crate::theme::{BORDER_WIDTH, RADIUS, SCROLL_FADE, ThemeStore, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    MenuItem, MenuRowHandle, MenuSheetHandle, MenuStyle, ScrollbarStyle,
};
use beui_core::base::TextAlign;
use beui_core::color::Color32;
use beui_core::geometry::Pos2;
use beui_core::icons::ICON_CHEVRON_RIGHT;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Align, Callback, Child, Children, ClickCallback, Direction, Frame, ItemSize, List, Prop, Show,
    create_memo,
};

const PADDING_HORIZONTAL: f32 = 14.0;
const PADDING_VERTICAL: f32 = 6.0;
const MENU_PADDING: f32 = 4.0;
const MENU_WIDTH: f32 = 200.0;
const SUBMENU_SPACING: f32 = 8.0;
const SHEET_PADDING: f32 = 8.0;
const SHEET_STOPS: [f32; 2] = [0.5, 0.9];
const SHEET_SEPARATOR_SPACING: f32 = 4.0;

#[component]
pub fn ContextMenu(
    children: Child,
    items: Children<MenuItem>,
    #[prop(default = ItemSize::Intrinsic)] child_size: Prop<ItemSize>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = None)] open_at: Prop<Option<Pos2>>,
    #[prop(default = true)] open_at_focuses: bool,
    #[prop(default = false)] open_at_pointer: Prop<bool>,
    #[prop(default = false)] selection: bool,
    on_close: ClickCallback,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    view! {
        <unstyled::ContextMenu
            items
            menu={menu_style()}
            child_size={child_size}
            disabled={disabled}
            open_at={open_at}
            open_at_focuses
            open_at_pointer
            selection
            on_close={on_close}
            on_select={on_select}
        >
            {children}
        </unstyled::ContextMenu>
    }
}

pub fn menu_style() -> MenuStyle {
    MenuStyle::new(
        |handle| {
            view! {
                <MenuRow handle />
            }
        },
        |content| {
            view! {
                <MenuPanel>{content}</MenuPanel>
            }
        },
    )
    .with_sheet(
        |handle| {
            view! {
                <MenuSheetRow handle />
            }
        },
        |handle| {
            view! {
                <MenuSheetPanel handle />
            }
        },
    )
}

#[component]
fn MenuRow(handle: MenuRowHandle) -> NodeId {
    let MenuRowHandle {
        label,
        glyph,
        detail,
        disabled,
        danger,
        separated,
        has_submenu,
        hovered,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let disabled = create_memo(move || disabled.get());
    let danger = create_memo(move || danger.get());
    let color = create_memo(move || match (disabled.get(), danger.get()) {
        (true, _) => TextColor::Muted,
        (false, true) => TextColor::Danger,
        (false, false) => TextColor::Default,
    });
    let icon_color = create_memo(move || match (disabled.get(), danger.get()) {
        (false, true) => TextColor::Danger,
        _ => TextColor::Muted,
    });
    let glyph = create_memo(move || glyph.get());
    let has_glyph = create_memo(move || !glyph.get().is_empty());
    let detail = create_memo(move || detail.get());
    let has_detail = create_memo(move || !detail.get().is_empty());
    let chevron_color = TextColor::Muted;
    let fill_color = create_memo(move || row_background(&theme, focused.get(), hovered.get()));
    view! {
        <List spacing=MENU_PADDING>
            <Show condition={separated}>
                <Separator />
            </Show>
            <Frame
                color={fill_color}
                radius=RADIUS
                padding_horizontal=PADDING_HORIZONTAL
                padding_vertical=PADDING_VERTICAL
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=SUBMENU_SPACING>
                    <Show condition={has_glyph}>
                        <Text color={icon_color} align=TextAlign::Center>
                            <Icon glyph={glyph} />
                        </Text>
                    </Show>
                    <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
                        <Text string={label} color align=TextAlign::Start ellipsis=true />
                        <Show condition={has_detail}>
                            <Text
                                string={detail}
                                size=TextSize::Sm
                                color=TextColor::Muted
                                align=TextAlign::Start
                                ellipsis=true
                            />
                        </Show>
                    </List>
                    <Frame visible={has_submenu}>
                        <Text size=TextSize::Sm color={chevron_color} align=TextAlign::Center>
                            <Icon glyph=ICON_CHEVRON_RIGHT />
                        </Text>
                    </Frame>
                </List>
            </Frame>
        </List>
    }
}

#[component]
fn MenuSheetRow(handle: MenuRowHandle) -> NodeId {
    let MenuRowHandle {
        label,
        glyph,
        detail,
        disabled,
        danger,
        separated,
        has_submenu,
        hovered,
        active,
        focused,
    } = handle;
    view! {
        <List spacing=SHEET_SEPARATOR_SPACING>
            <Show condition={separated}>
                <Separator />
            </Show>
            <ActionRowFace
                hovered
                active
                focused
                label
                glyph
                detail
                disabled
                danger
                submenu={has_submenu}
            />
        </List>
    }
}

#[component]
fn MenuSheetPanel(handle: MenuSheetHandle) -> NodeId {
    let MenuSheetHandle {
        open,
        on_close,
        menu,
    } = handle;
    view! {
        <ModalSheet
            open={open}
            rest={SHEET_STOPS[0]}
            stops={SHEET_STOPS.to_vec()}
            on_close={on_close}
        >
            <Frame padding_horizontal=SHEET_PADDING padding_vertical=SHEET_PADDING>{menu}</Frame>
        </ModalSheet>
    }
}

#[component]
fn MenuPanel(children: Child) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            width=MENU_WIDTH
            color={theme.surface_raised}
            outline={theme.border}
            outline_width=BORDER_WIDTH
            radius=RADIUS
            outline_visible=true
        >
            <unstyled::Scroll
                fit=true
                tab_stop=false
                scrollbar={ScrollbarStyle::default().fading(SCROLL_FADE)}
            >
                <Frame padding_horizontal=MENU_PADDING padding_vertical=MENU_PADDING>
                    {children}
                </Frame>
            </unstyled::Scroll>
        </Frame>
    }
}

fn row_background(theme: &ThemeStore, focused: bool, hovered: bool) -> Color32 {
    match (focused, hovered) {
        (true, _) => theme.accent_soft.get(),
        (false, true) => theme.pressed.get(),
        (false, false) => Color32::TRANSPARENT,
    }
}
