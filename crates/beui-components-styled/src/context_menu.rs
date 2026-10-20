use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::action_row::ActionRowFace;
use crate::border::Separator;
use crate::button::{ButtonFace, ButtonVariant};
use crate::sheet::ModalSheet;
use crate::text::{Icon, Text, TextColor, TextSize};
use crate::theme::{SCROLL_FADE, ThemeStore, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    ButtonHandle, MenuItem, MenuRowHandle, MenuSheetHandle, MenuStyle, ScrollbarStyle,
};
use beui_core::base::TextAlign;
use beui_core::color::Color32;
use beui_core::geometry::{Pos2, Rect};
use beui_core::icons::{ICON_CHECK, ICON_CHEVRON_RIGHT};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

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
    #[prop(default = None)] toolbar_at: Prop<Option<Rect>>,
    #[prop(default = true)] opens_on_hold: bool,
    on_close: ClickCallback,
    on_select: Callback<Vec<usize>>,
    on_toolbar_close: ClickCallback,
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
            toolbar_at
            opens_on_hold
            on_close={on_close}
            on_select={on_select}
            on_toolbar_close={on_toolbar_close}
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
    .with_toolbar(
        |handle| {
            view! {
                <ToolbarButton handle />
            }
        },
        |content| {
            view! {
                <ToolbarPanel>{content}</ToolbarPanel>
            }
        },
    )
}

#[component]
fn ToolbarButton(handle: ButtonHandle) -> NodeId {
    let glyph = handle.glyph;
    let icon_only = create_memo(move || !glyph.get().is_empty());
    view! {
        <ButtonFace handle variant=ButtonVariant::Ghost icon_only />
    }
}

#[component]
fn ToolbarPanel(children: Child) -> NodeId {
    view! {
        <Frame
            fill=ColorRole::SurfaceRaised
            border={Border::DEFAULT}
            radius=Radius::Md
            padding=MENU_PADDING
        >
            {children}
        </Frame>
    }
}

#[component]
fn MenuRow(handle: MenuRowHandle) -> NodeId {
    let MenuRowHandle {
        label,
        glyph,
        detail,
        checked,
        disabled,
        danger,
        separated,
        has_submenu,
        hovered,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let ticked = create_memo(move || checked.get() == Some(true));
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
                fill={ColorRole::custom(fill_color)}
                radius=Radius::Md
                padding={Sides::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
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
                    <Show condition={ticked}>
                        <Text color=TextColor::Accent align=TextAlign::Center>
                            <Icon glyph=ICON_CHECK />
                        </Text>
                    </Show>
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
        checked,
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
                checked
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
            <Frame padding=SHEET_PADDING>{menu}</Frame>
        </ModalSheet>
    }
}

#[component]
fn MenuPanel(children: Child) -> NodeId {
    view! {
        <Frame
            width=MENU_WIDTH
            fill=ColorRole::SurfaceRaised
            border={Border::DEFAULT}
            radius=Radius::Md
        >
            <unstyled::Scroll
                fit=true
                tab_stop=false
                scrollbar={ScrollbarStyle::default().fading(SCROLL_FADE)}
            >
                <Frame padding=MENU_PADDING>{children}</Frame>
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
