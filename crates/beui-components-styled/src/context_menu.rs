use beui_macros::{component, view};

use crate::action_row::ActionRowFace;
use crate::border::Separator;
use crate::sheet::ModalSheet;
use crate::text::IconSized;
use crate::theme::{BORDER_WIDTH, FONT_BODY, FONT_SMALL, RADIUS, ThemeStore, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{MenuItem, MenuRowHandle, MenuSheetHandle, MenuStyle};
use beui_core::base::TextAlign;
use beui_core::color::Color32;
use beui_core::geometry::Pos2;
use beui_core::icons::ICON_CHEVRON_RIGHT;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Align, Callback, Child, Children, ClickCallback, Direction, Frame, ItemSize, List, Prop, Show,
    Text, clone, create_memo,
};

const PADDING_HORIZONTAL: f32 = 14.0;
const PADDING_VERTICAL: f32 = 6.0;
const MENU_PADDING: f32 = 4.0;
const MENU_WIDTH: f32 = 200.0;
const SUBMENU_SPACING: f32 = 8.0;
const SUBMENU_ICON_SIZE: f32 = 16.0;
const ROW_ICON_SIZE: f32 = 18.0;
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
            on_close={move || on_close.call()}
            on_select={move |path| on_select.call(path)}
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
    let color = create_memo(clone!(theme disabled danger -> move || {
        match (disabled.get(), danger.get()) {
            (true, _) => theme.text_muted.get(),
            (false, true) => theme.danger.get(),
            (false, false) => theme.text.get(),
        }
    }));
    let icon_color = create_memo(clone!(theme disabled danger -> move || {
        match (disabled.get(), danger.get()) {
            (false, true) => theme.danger.get(),
            _ => theme.text_muted.get(),
        }
    }));
    let glyph = create_memo(move || glyph.get());
    let has_glyph = create_memo(clone!(glyph -> move || !glyph.get().is_empty()));
    let detail = create_memo(move || detail.get());
    let has_detail = create_memo(clone!(detail -> move || !detail.get().is_empty()));
    let chevron_color = theme.text_muted.clone();
    let fill_color =
        create_memo(clone!(theme -> move || row_background(&theme, focused.get(), hovered.get())));
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
                        <IconSized
                            glyph={glyph.clone()}
                            font_size=ROW_ICON_SIZE
                            color={icon_color.clone()}
                        />
                    </Show>
                    <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
                        <Text
                            string={label}
                            font_size=FONT_BODY
                            color
                            align=TextAlign::Start
                            ellipsis=true
                        />
                        <Show condition={has_detail}>
                            <Text
                                string={detail.clone()}
                                font_size=FONT_SMALL
                                color={theme.text_muted.clone()}
                                align=TextAlign::Start
                                ellipsis=true
                            />
                        </Show>
                    </List>
                    <Frame visible={has_submenu}>
                        <IconSized
                            glyph=ICON_CHEVRON_RIGHT
                            font_size=SUBMENU_ICON_SIZE
                            color={chevron_color}
                        />
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
            on_close={move || on_close.call()}
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
            color={theme.surface_raised.clone()}
            outline={theme.border.clone()}
            outline_width=BORDER_WIDTH
            radius=RADIUS
            outline_visible=true
            padding_horizontal=MENU_PADDING
            padding_vertical=MENU_PADDING
        >
            {children}
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
