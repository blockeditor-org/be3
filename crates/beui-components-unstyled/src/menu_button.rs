use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate as unstyled;
use crate::menu::{MenuItem, MenuList, MenuRowHandle};
use beui_core::base::overlay::Placement;
use beui_core::input::PointerPress;
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Callback, Child, Children, ClickCallback, List, Memo, NodeRef, Prop, ReadSignal, Render,
    RenderFn, Show, clone, create_memo, create_signal,
};

#[derive(Clone)]
pub struct MenuButtonHandle {
    pub open: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub struct MenuSheetHandle {
    pub open: Memo<bool>,
    pub on_close: ClickCallback,
    pub menu: Child,
}

#[derive(Clone)]
pub struct MenuSheet {
    pub row: RenderFn<MenuRowHandle>,
    pub sheet: RenderFn<MenuSheetHandle>,
}

#[component]
pub fn MenuButton(
    items: Children<MenuItem>,
    trigger: Render<MenuButtonHandle>,
    row: Option<RenderFn<MenuRowHandle>>,
    panel: Option<RenderFn<Child>>,
    sheet: Option<MenuSheet>,
    #[prop(default = false)] disabled: Prop<bool>,
    accessibility: Option<Prop<Node>>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::Button)));
    let row = row.expect("a menu button needs a `row` builder");
    let panel = panel.expect("a menu button needs a `panel` builder");
    let (open, set_open) = create_signal(false);
    let (touched, set_touched) = create_signal(false);
    let sheeted = sheet.is_some();
    let dropped = create_memo(clone!(open touched -> move || {
        open.get() && !(sheeted && touched.get())
    }));
    let raised = create_memo(clone!(open -> move || sheeted && open.get() && touched.get()));
    let button = NodeRef::new();
    let items = items.into_run();
    let sheet_items = items.clone();
    let sheet_panel = panel.clone();
    let sheet_select = on_select.clone();
    let closing = set_open.clone();
    let shown = dropped.clone();
    view! {
        <List spacing=0.0>
            <unstyled::Button
                @node_ref=&button
                disabled={disabled}
                accessibility={accessibility}
                on_press={move |press: PointerPress| set_touched.set(press.touch)}
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
                {panel.call(view! {
                    <MenuList
                        items={items}
                        row
                        panel={panel.clone()}
                        active={dropped.clone()}
                        on_select={clone!(set_open -> move |path: Vec<usize>| {
                            on_select.call(path);
                            set_open.set(false);
                        })}
                    />
                })}
            </Overlay>
            <Show condition={sheeted}>
                {move || {
                    let sheet = sheet.clone().unwrap_or_else(|| unreachable!());
                    let select = sheet_select.clone();
                    let dismiss = closing.clone();
                    let closing = closing.clone();
                    sheet.sheet.call(MenuSheetHandle {
                        open: raised.clone(),
                        on_close: ClickCallback::new(move || dismiss.set(false)),
                        menu: view! {
                            <MenuList
                                items={sheet_items.clone()}
                                row={sheet.row.clone()}
                                panel={sheet_panel.clone()}
                                active={raised.clone()}
                                on_select={move |path: Vec<usize>| {
                                    select.call(path);
                                    closing.set(false);
                                }}
                            />
                        },
                    })
                }}
            </Show>
        </List>
    }
}
