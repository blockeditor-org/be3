use beui_macros::{component, view};

use crate::base::TextAlign;
use crate::color::Color32;
use crate::node::NodeId;
use crate::reactive::{create_memo, Callback, Child, Frame, Prop, Text};
use crate::styled::theme::{
    ACCENT_SOFT, BORDER, BORDER_WIDTH, FONT_BODY, RADIUS, SURFACE_RAISED, TEXT, TEXT_MUTED,
};
use crate::unstyled;
use crate::unstyled::{MenuItem, MenuRowHandle};

const PADDING_HORIZONTAL: f32 = 14.0;
const PADDING_VERTICAL: f32 = 6.0;
const MENU_PADDING: f32 = 4.0;
const MENU_WIDTH: f32 = 200.0;

#[component]
pub fn ContextMenu(
    children: Child,
    items: Prop<Vec<MenuItem>>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    view! {
        <unstyled::ContextMenu
            items
            row={|handle| view! { <MenuRow handle /> }}
            panel={|content| view! { <MenuPanel>{content}</MenuPanel> }}
            on_select={move |path| on_select.call(path)}
        >
            {children}
        </unstyled::ContextMenu>
    }
}

#[component]
fn MenuRow(handle: MenuRowHandle) -> NodeId {
    let MenuRowHandle {
        item,
        hovered,
        focused,
    } = handle;
    let color = if item.disabled { TEXT_MUTED } else { TEXT };
    let fill_color = create_memo(move || row_background(focused.get(), hovered.get()));
    view! {
        <Frame color={fill_color} radius=RADIUS padding_horizontal=PADDING_HORIZONTAL padding_vertical=PADDING_VERTICAL>
            <Text
                string={item.label}
                font_size=FONT_BODY
                color
                align=TextAlign::Start
            />
        </Frame>
    }
}

#[component]
fn MenuPanel(children: Child) -> NodeId {
    view! {
        <Frame width=MENU_WIDTH color=SURFACE_RAISED outline=BORDER outline_width=BORDER_WIDTH radius=RADIUS outline_visible=true padding_horizontal=MENU_PADDING padding_vertical=MENU_PADDING>
            {children}
        </Frame>
    }
}

fn row_background(focused: bool, hovered: bool) -> Color32 {
    match (focused, hovered) {
        (true, _) => ACCENT_SOFT,
        (false, true) => BORDER,
        (false, false) => Color32::TRANSPARENT,
    }
}
