use accesskit::{HasPopup, Node, Role};
use beui_macros::{component, view};

use crate as unstyled;
use crate::menu::MenuItem;
use crate::menu_popup::{MenuPopup, MenuStyle};
use beui_core::input::PointerPress;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, Children, List, NodeRef, Prop, ReadSignal, Render, clone, create_memo, create_signal,
};

#[derive(Clone)]
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
    accessibility: Option<Prop<Node>>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::Button)));
    let (open, set_open) = create_signal(false);
    let accessibility = create_memo(clone!(open -> move || {
        let mut node = accessibility.get();
        node.set_has_popup(HasPopup::Menu);
        node.set_expanded(open.get());
        node
    }));
    let (touched, set_touched) = create_signal(false);
    let button = NodeRef::new();
    let shown = create_memo(clone!(open -> move || open.get()));
    let touched = create_memo(move || touched.get());
    view! {
        <List spacing=0.0>
            <unstyled::Button
                @node_ref=&button
                label
                glyph
                disabled={disabled}
                accessibility={accessibility}
                on_press={move |press: PointerPress| set_touched.set(press.touch)}
                on_click={clone!(set_open -> move || set_open.update(|open| *open = !*open))}
                content={Render::new(clone!(open -> move |handle: unstyled::ButtonHandle| {
                    trigger.call(MenuButtonHandle {
                        open: open.clone(),
                        button: handle,
                    })
                }))}
            />
            <MenuPopup
                items={items.into_run()}
                menu
                anchor={&button}
                open={shown}
                touched
                on_dismiss={move || set_open.set(false)}
                on_select={move |path: Vec<usize>| on_select.call(path)}
            />
        </List>
    }
}
