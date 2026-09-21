use beui_macros::{component, view};

use crate::base::overlay::{Overlay, Placement};
use crate::node::NodeId;
use crate::reactive::{
    Callback, Child, Children, List, NodeRef, Prop, ReadSignal, Render, RenderFn, clone,
    create_signal,
};
use crate::unstyled;
use crate::unstyled::menu::{MenuItem, MenuList, MenuRowHandle};

#[derive(Clone)]
pub struct MenuButtonHandle {
    pub open: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

#[component]
pub fn MenuButton(
    items: Children<MenuItem>,
    trigger: Render<MenuButtonHandle>,
    row: Option<RenderFn<MenuRowHandle>>,
    panel: Option<RenderFn<Child>>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let row = row.expect("a menu button needs a `row` builder");
    let panel = panel.expect("a menu button needs a `panel` builder");
    let (open, set_open) = create_signal(false);
    let button = NodeRef::new();
    let items = items.into_run();
    let shown = open.clone();
    let menu = panel.call(view! {
        <MenuList
            items={items}
            row
            panel={panel.clone()}
            active={open.clone()}
            on_select={clone!(set_open -> move |path: Vec<usize>| {
                on_select.call(path);
                set_open.set(false);
            })}
        />
    });
    view! {
        <List spacing=0.0>
            <unstyled::Button
                @node_ref=&button
                disabled={disabled}
                on_click={clone!(set_open -> move || set_open.update(|open| *open = !*open))}
                content={Render::new(clone!(open -> move |handle: unstyled::ButtonHandle| {
                    trigger.call(MenuButtonHandle {
                        open: open.clone(),
                        hovered: handle.hovered,
                        active: handle.active,
                        focused: handle.focused,
                    })
                }))}
            />
            <Overlay
                anchor={&button}
                placement=Placement::BelowStart
                open={shown}
                on_dismiss={clone!(set_open -> move || set_open.set(false))}
            >
                {menu}
            </Overlay>
        </List>
    }
}
