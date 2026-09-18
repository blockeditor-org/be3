use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::color::Color32;
use crate::icons::ICON_CHEVRON_RIGHT;
use crate::node::NodeId;
use crate::reactive::{
    Align, Callback, Child, Children, Direction, Frame, ItemSize, List, Text, clone, create_memo,
};
use crate::styled::text::IconSized;
use crate::styled::theme::{BORDER_WIDTH, FONT_BODY, RADIUS, ThemeStore, use_theme};
use crate::unstyled;
use crate::unstyled::{MenuItem, MenuRowHandle, TextInputMenu};

const PADDING_HORIZONTAL: f32 = 14.0;
const PADDING_VERTICAL: f32 = 6.0;
const MENU_PADDING: f32 = 4.0;
const MENU_WIDTH: f32 = 200.0;
const SUBMENU_SPACING: f32 = 8.0;
const SUBMENU_ICON_SIZE: f32 = 16.0;

#[component]
pub fn ContextMenu(
    children: Child,
    items: Children<MenuItem>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    view! {
        <unstyled::ContextMenu
            items
            row={|handle| view! {
                <MenuRow handle />
            }}
            panel={|content| view! {
                <MenuPanel>{content}</MenuPanel>
            }}
            on_select={move |path| on_select.call(path)}
        >
            {children}
        </unstyled::ContextMenu>
    }
}

pub(crate) fn text_input_menu() -> TextInputMenu {
    TextInputMenu::new(
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
}

#[component]
fn MenuRow(handle: MenuRowHandle) -> NodeId {
    let MenuRowHandle {
        label,
        disabled,
        has_submenu,
        hovered,
        focused,
    } = handle;
    let theme = use_theme();
    let color = create_memo(clone!(theme -> move || {
        if disabled.get() {
            theme.text_muted.get()
        } else {
            theme.text.get()
        }
    }));
    let arrow_color = color.clone();
    let fill_color =
        create_memo(clone!(theme -> move || row_background(&theme, focused.get(), hovered.get())));
    view! {
        <Frame
            color={fill_color}
            radius=RADIUS
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=PADDING_VERTICAL
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=SUBMENU_SPACING>
                <Text
                    @sizing=ItemSize::Percent(100.0)
                    string={label}
                    font_size=FONT_BODY
                    color
                    align=TextAlign::Start
                />
                <Frame visible={has_submenu}>
                    <IconSized
                        glyph=ICON_CHEVRON_RIGHT
                        font_size=SUBMENU_ICON_SIZE
                        color={arrow_color}
                    />
                </Frame>
            </List>
        </Frame>
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
