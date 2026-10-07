use std::rc::Rc;

use beui_macros::{component, view};

use crate as unstyled;
use crate::button::ButtonHandle;
use crate::menu::{MenuItem, MenuList, MenuRowHandle};
use beui_core::base::overlay::{OverlayAnchor, OverlayMode, Placement};
use beui_core::document::Document;
use beui_core::geometry::{Pos2, Rect, pos2};
use beui_core::icons::ICON_MORE_HORIZ;
use beui_core::input::PointerPress;
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Align, Callback, Child, Children, ClickCallback, Direction, Interactive, ItemSize, List, Memo,
    NodeRef, Prop, RenderFn, Run, clone, create_effect, create_memo, create_signal,
    set_component_state,
};

struct State {
    overlay: NodeRef,
    content: NodeRef,
    toolbar: NodeRef,
    more: NodeRef,
}

type Parts<H> = (RenderFn<H>, RenderFn<Child>);

#[derive(Clone, Default)]
pub struct MenuStyle {
    menu: Option<Parts<MenuRowHandle>>,
    toolbar: Option<Parts<ButtonHandle>>,
}

impl MenuStyle {
    pub fn new(
        row: impl Fn(MenuRowHandle) -> NodeId + 'static,
        panel: impl Fn(Child) -> NodeId + 'static,
    ) -> Self {
        Self {
            menu: Some((RenderFn::new(row), RenderFn::new(panel))),
            toolbar: None,
        }
    }

    pub fn with_toolbar(
        self,
        button: impl Fn(ButtonHandle) -> NodeId + 'static,
        panel: impl Fn(Child) -> NodeId + 'static,
    ) -> Self {
        Self {
            toolbar: Some((RenderFn::new(button), RenderFn::new(panel))),
            ..self
        }
    }

    pub fn is_some(&self) -> bool {
        self.menu.is_some()
    }

    pub fn has_toolbar(&self) -> bool {
        self.toolbar.is_some()
    }

    fn toolbar_parts(&self) -> Parts<ButtonHandle> {
        self.toolbar.clone().unwrap_or_else(|| {
            (
                RenderFn::new(|_| {
                    view! {
                        <List spacing=0.0 />
                    }
                }),
                RenderFn::new(|content| content),
            )
        })
    }

    pub fn parts(self) -> (RenderFn<MenuRowHandle>, RenderFn<Child>) {
        self.menu.unwrap_or_else(|| {
            (
                RenderFn::new(|_| {
                    view! {
                        <List spacing=0.0 />
                    }
                }),
                RenderFn::new(|content| content),
            )
        })
    }
}

#[component]
pub fn ContextMenu(
    children: Child,
    items: Children<MenuItem>,
    #[prop(default = MenuStyle::default())] menu: MenuStyle,
    #[prop(default = ItemSize::Intrinsic)] child_size: Prop<ItemSize>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = None)] open_at: Prop<Option<Pos2>>,
    #[prop(default = true)] open_at_focuses: bool,
    #[prop(default = None)] toolbar_at: Prop<Option<Rect>>,
    #[prop(default = true)] opens_on_hold: bool,
    on_close: ClickCallback,
    on_select: Callback<Vec<usize>>,
    on_toolbar_close: ClickCallback,
) -> NodeId {
    let has_toolbar = menu.has_toolbar();
    let (button, bar) = menu.toolbar_parts();
    let (row, panel) = menu.parts();
    let (open, set_open) = create_signal(false);
    let (focusing, set_focusing) = create_signal(true);
    let active = create_memo(clone!(open focusing -> move || open.get() && focusing.get()));
    let pressed_focusing = set_focusing.clone();
    let (position, set_position) = create_signal(Pos2::ZERO);
    let anchor = create_memo(move || OverlayAnchor::Point(position.get()));
    let (overlay, content, toolbar) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let more_button = NodeRef::new();
    set_component_state(State {
        overlay: overlay.clone(),
        content: content.clone(),
        toolbar: toolbar.clone(),
        more: more_button.clone(),
    });

    let dismiss = set_open.clone();
    let close = set_open.clone();
    let dismissed = on_close.clone();
    let requested = set_open.clone();
    let requested_position = set_position.clone();
    let requested_focusing = set_focusing.clone();
    create_effect(move || {
        let Some(at) = open_at.get() else {
            return;
        };
        requested_position.set(at);
        requested_focusing.set(open_at_focuses);
        requested.set(true);
    });
    let shown_at = toolbar_at.clone();
    let toolbar_open = create_memo(clone!(open disabled -> move || {
        has_toolbar && shown_at.get().is_some() && !open.get() && !disabled.get()
    }));
    let toolbar_anchor = create_memo(clone!(toolbar_at -> move || {
        OverlayAnchor::Rect(toolbar_at.get().unwrap_or(Rect::NOTHING))
    }));
    let more = Callback::new(
        clone!(set_position set_focusing set_open on_toolbar_close -> move |at: Pos2| {
            set_position.set(at);
            set_focusing.set(false);
            set_open.set(true);
            on_toolbar_close.call();
        }),
    );
    let fallback = more.clone();
    create_effect(move || {
        if has_toolbar {
            return;
        }
        if let Some(rect) = toolbar_at.get() {
            fallback.call(pos2(rect.center().x, rect.bottom()));
        }
    });
    let chosen = on_select.clone();
    let choose = Callback::new(move |index: usize| {
        on_toolbar_close.call();
        chosen.call(vec![index]);
    });
    let items = items.into_run();
    let toolbar_items = items.clone();
    view! {
        <Interactive
            on_secondary_press={move |press: PointerPress| {
                if disabled.get() || (press.touch && !opens_on_hold) {
                    return;
                }
                set_position.set(press.pos);
                pressed_focusing.set(true);
                set_open.set(true);
            }}
        >
            <List spacing=0.0>
                {children} @sizing={child_size}
                <Overlay
                    @node_ref=&overlay
                    anchor
                    placement=Placement::BelowStart
                    traps_focus={focusing}
                    open
                    on_dismiss={move || {
                        dismiss.set(false);
                        dismissed.call();
                    }}
                >
                    {panel.call(view! {
                        <MenuList
                            @node_ref=&content
                            items={items}
                            row
                            panel={panel.clone()}
                            active
                            on_select={move |path: Vec<usize>| {
                                on_select.call(path);
                                close.set(false);
                                on_close.call();
                            }}
                        />
                    })}
                </Overlay>
                <Toolbar
                    @node_ref=&toolbar
                    items={toolbar_items}
                    button
                    panel={bar}
                    anchor={toolbar_anchor}
                    open={toolbar_open}
                    more={more_button}
                    on_choose={move |index: usize| choose.call(index)}
                    on_more={move |at: Pos2| more.call(at)}
                />
            </List>
        </Interactive>
    }
}

#[component]
fn Toolbar(
    items: Run<MenuItem>,
    button: RenderFn<ButtonHandle>,
    panel: RenderFn<Child>,
    anchor: Memo<OverlayAnchor>,
    open: Memo<bool>,
    more: NodeRef,
    on_choose: Callback<usize>,
    on_more: Callback<Pos2>,
) -> NodeId {
    let (row, chosen) = (button.clone(), on_choose);
    let buttons = items.build(move |items: Vec<Rc<MenuItem>>| {
        items
            .into_iter()
            .enumerate()
            .filter(|(_, item)| item.children.peek().is_empty())
            .map(|(index, item)| {
                let chosen = chosen.clone();
                view! {
                    <ToolbarButton
                        item
                        index
                        button={row.clone()}
                        on_choose={move |index: usize| chosen.call(index)}
                    />
                }
            })
            .collect::<Vec<NodeId>>()
    });
    view! {
        <Overlay
            anchor
            placement=Placement::Above
            mode=OverlayMode::Floating
            traps_focus=false
            open
        >
            {panel.call(view! {
                <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                    {buttons}
                    <unstyled::Button
                        @node_ref=&more
                        label="More"
                        glyph=ICON_MORE_HORIZ
                        tab_stop=false
                        press_focus=false
                        content={move |handle: ButtonHandle| button.call(handle)}
                        on_click_at={move |press: PointerPress| on_more.call(press.pos)}
                    />
                </List>
            })}
        </Overlay>
    }
}

#[component]
fn ToolbarButton(
    item: Rc<MenuItem>,
    index: usize,
    button: RenderFn<ButtonHandle>,
    on_choose: Callback<usize>,
) -> NodeId {
    let disabled = item.disabled.clone();
    let action = item.action.clone();
    view! {
        <unstyled::Button
            label={item.label.clone()}
            disabled={item.disabled.clone()}
            tab_stop=false
            press_focus=false
            content={move |handle: ButtonHandle| button.call(handle)}
            on_click={move || {
                if disabled.get_untracked() {
                    return;
                }
                on_choose.call(index);
                if let Some(action) = &action {
                    action.run();
                }
            }}
        />
    }
}

pub fn context_menu_menu(document: &Document, context_menu: NodeId) -> NodeId {
    document
        .component_state::<State>(context_menu)
        .content
        .get()
}

pub fn context_menu_overlay(document: &Document, context_menu: NodeId) -> NodeId {
    document
        .component_state::<State>(context_menu)
        .overlay
        .get()
}

pub fn context_menu_toolbar(document: &Document, context_menu: NodeId) -> NodeId {
    document
        .component_state::<State>(context_menu)
        .toolbar
        .get()
}

pub fn context_menu_more(document: &Document, context_menu: NodeId) -> NodeId {
    document.component_state::<State>(context_menu).more.get()
}
