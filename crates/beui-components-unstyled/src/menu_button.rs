use accesskit::{HasPopup, Node, Role};
use beui_macros::{component, view};

use crate as unstyled;
use crate::menu::MenuItem;
use crate::menu_popup::{MenuPopup, MenuStyle};
use beui_core::input::{PointerPress, ScrollGesture};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

#[derive(Clone, Copy)]
pub struct MenuButtonHandle {
    pub open: ReadSignal<bool>,
    pub button: unstyled::ButtonHandle,
}

#[component]
pub fn MenuButton(
    items: Children<MenuItem>,
    trigger: Render<MenuButtonHandle>,
    #[prop(default = MenuStyle::default())] menu: MenuStyle,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = true)] click_opens: Prop<bool>,
    #[prop(default = false)] secondary_opens: Prop<bool>,
    accessibility: Option<Prop<Node>>,
    on_click: ClickCallback,
    on_secondary_click: Option<ClickCallback>,
    on_middle_click: Option<ClickCallback>,
    on_scroll: Option<Callback<ScrollGesture>>,
    on_open_change: Callback<bool>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::Button)));
    let open = create_signal(false);
    let accessibility = create_memo(move || {
        let mut node = accessibility.get();
        node.set_has_popup(HasPopup::Menu);
        node.set_expanded(open.get());
        node
    });
    let touched = create_signal(false);
    let button = NodeRef::new();
    let shown = create_memo(move || open.get());
    if !on_open_change.is_empty() {
        let mut was_open = false;
        create_effect(move || {
            let open = shown.get();
            if open != was_open {
                was_open = open;
                on_open_change.call(open);
            }
        });
    }
    let disabled = create_memo(move || disabled.get());
    let mut trigger_node = view! {
        <unstyled::Button
            @node_ref=&button
            label
            glyph
            disabled={disabled}
            accessibility={accessibility}
            on_press={move |press: PointerPress| touched.set(press.touch)}
            on_click={move || {
                match untrack(|| click_opens.get()) {
                    true => open.update(|open| *open = !*open),
                    false => on_click.call(),
                }
            }}
            content={Render::new(move |handle: unstyled::ButtonHandle| {
                trigger.call(MenuButtonHandle {
                    open: open.read_only(),
                    button: handle,
                })
            })}
        />
    };
    if on_secondary_click.is_some() || !matches!(secondary_opens, Prop::Static(false)) {
        let on_secondary_click = on_secondary_click.unwrap_or_default();
        trigger_node = view! {
            <Interactive
                on_secondary_press={move |press: PointerPress| {
                    if untrack(|| disabled.get()) {
                        return;
                    }
                    match untrack(|| secondary_opens.get()) {
                        true => {
                            touched.set(press.touch);
                            open.set(true);
                        }
                        false => on_secondary_click.call(),
                    }
                }}
            >
                {trigger_node}
            </Interactive>
        };
    }
    if let Some(on_middle_click) = on_middle_click {
        trigger_node = view! {
            <Interactive
                on_middle_click={move || {
                    if !untrack(|| disabled.get()) {
                        on_middle_click.call();
                    }
                }}
            >
                {trigger_node}
            </Interactive>
        };
    }
    if let Some(on_scroll) = on_scroll {
        trigger_node = view! {
            <Interactive
                on_scroll={move |gesture: ScrollGesture| {
                    if !untrack(|| disabled.get()) {
                        on_scroll.call(gesture);
                    }
                }}
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
                touched={create_memo(move || touched.get())}
                on_dismiss={move || open.set(false)}
                on_select={on_select}
            />
        </List>
    }
}
