use accesskit::{HasPopup, Node, Role};
use beui_macros::{component, view};
use beui_view::types::icons::Glyph;

use crate::menu::MenuItem;
use crate::menu_popup::{MenuPopup, MenuStyle};
use crate::popup_trigger::{PopupTrigger, PopupTriggerHandle};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::{PointerPress, ScrollGesture};

#[component]
pub fn MenuButton(
    items: Children<MenuItem>,
    trigger: Render<PopupTriggerHandle>,
    menu: MenuStyle,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = true)] click_opens: Prop<bool>,
    #[prop(default = false)] secondary_opens: Prop<bool>,
    #[prop(default = true)] tab_stop: Prop<bool>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    accessibility: Option<Prop<Node>>,
    on_click: ClickCallback,
    on_secondary_click: ClickCallback,
    on_middle_click: ClickCallback,
    on_scroll: Callback<ScrollGesture>,
    #[prop(default = false)] open: Binding<bool>,
    #[prop(default = Callback::default())] on_select: Callback<Vec<usize>>,
) -> NodeId {
    let button = NodeRef::new();
    let shown = create_memo(move || open.get());
    let disabled = create_memo(move || disabled.get());
    let mut trigger_node = view! {
        <PopupTrigger
            @node_ref=&button
            open
            popup=HasPopup::Menu
            label
            glyph
            icon_only
            disabled={disabled}
            click_opens
            tab_stop
            capture_presses
            accessibility={accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::Button)))}
            on_click={on_click}
            content={trigger}
        />
    };
    if !on_secondary_click.is_empty() || !matches!(secondary_opens, Prop::Static(false)) {
        trigger_node = view! {
            <Interactive
                on_secondary_press={move |_: PointerPress| {
                    if disabled.get_untracked() {
                        return;
                    }
                    match secondary_opens.get_untracked() {
                        true => open.set(true),
                        false => on_secondary_click.call(),
                    }
                }}
            >
                {trigger_node}
            </Interactive>
        };
    }
    if !on_middle_click.is_empty() {
        trigger_node = view! {
            <Interactive
                on_middle_click={move || {
                    if !disabled.get_untracked() {
                        on_middle_click.call();
                    }
                }}
            >
                {trigger_node}
            </Interactive>
        };
    }
    if !on_scroll.is_empty() {
        trigger_node = view! {
            <Interactive
                on_wheel={crate::gestures::scroll_wheel(None, move |gesture: ScrollGesture| {
                    if !disabled.get_untracked() {
                        on_scroll.call(gesture);
                    }
                })}
            >
                {trigger_node}
            </Interactive>
        };
    }
    view! {
        <List spacing=0.0>
            {trigger_node}
            <MenuPopup
                items={items.into_run()}
                menu
                anchor={button}
                open={shown}
                on_dismiss={move || open.set(false)}
                on_select={on_select}
            />
        </List>
    }
}
