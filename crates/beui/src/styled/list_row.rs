use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::color::Color32;

use crate::input::{Key, KeyPress, PointerPress};
use crate::node::NodeId;
use crate::reactive::{Child, ClickCallback, Frame, Prop, clone, create_memo};
use crate::styled::theme::{RADIUS, ThemeStore, use_theme};
use crate::unstyled::{Button, ButtonHandle};

const PADDING_HORIZONTAL: f32 = 8.0;
const PADDING_VERTICAL: f32 = 4.0;

#[component]
pub fn ListRow(
    children: Child,
    #[prop(default = false)] selected: Prop<bool>,
    on_click: ClickCallback,
    on_activate: ClickCallback,
) -> NodeId {
    let double_click = on_activate.clone();
    let selected = create_memo(move || selected.get());
    let accessibility = create_memo(clone!(selected -> move || {
        let mut node = Node::new(Role::Button);
        node.set_selected(selected.get());
        node
    }));
    view! {
        <Button
            accessibility
            on_click={move || on_click.call()}
            on_click_at={move |press: PointerPress| {
                if press.clicks >= 2 {
                    double_click.call();
                }
            }}
            on_key={move |press: KeyPress| {
                if on_activate.is_empty() || press.key != Key::Enter {
                    return false;
                }
                if press.pressed {
                    on_activate.call();
                }
                true
            }}
            content={move |handle| view! {
                <ListRowFace handle selected>{children}</ListRowFace>
            }}
        />
    }
}

#[component]
fn ListRowFace(handle: ButtonHandle, selected: Prop<bool>, children: Child) -> NodeId {
    let ButtonHandle {
        hovered,
        active,
        focused,
    } = handle;
    let theme = use_theme();
    let fill_color = create_memo(clone!(theme -> move || {
        background(&theme, selected.get(), hovered.get(), active.get())
    }));
    view! {
        <Frame
            color={fill_color}
            outline={theme.accent.clone()}
            outline_width=2.0
            radius=RADIUS
            outline_visible={focused}
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=PADDING_VERTICAL
        >
            {children}
        </Frame>
    }
}

fn background(theme: &ThemeStore, selected: bool, hovered: bool, active: bool) -> Color32 {
    match (selected, hovered, active) {
        (_, _, true) => theme.pressed.get(),
        (_, true, false) => theme.hover.get(),
        (true, false, false) => theme.accent_soft.get(),
        (false, false, false) => Color32::TRANSPARENT,
    }
}
