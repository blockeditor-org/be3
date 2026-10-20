use std::cell::RefCell;
use std::rc::Rc;

use beui_macros::{component, view};

use crate as unstyled;
use crate::button::ButtonHandle;
use crate::menu::MenuItem;
use crate::menu_popup::{MenuPopup, MenuStyle};
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::document::Document;
use beui_core::geometry::{Pos2, Rect, pos2};
use beui_core::icons::ICON_MORE_HORIZ;
use beui_core::input::PointerPress;
use beui_core::layer::{Dismiss, LayerPolicy};
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;

struct State {
    overlay: NodeRef,
    content: NodeRef,
    toolbar: NodeRef,
    more: NodeRef,
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
    #[prop(default = false)] open_at_pointer: Prop<bool>,
    #[prop(default = false)] selection: bool,
    #[prop(default = None)] toolbar_at: Prop<Option<Rect>>,
    #[prop(default = true)] opens_on_hold: bool,
    on_close: ClickCallback,
    on_select: Callback<Vec<usize>>,
    on_toolbar_close: ClickCallback,
) -> NodeId {
    let open = create_signal(false);
    let touched = create_signal(false);
    let focusing = create_signal(true);
    let position = create_signal(Pos2::ZERO);
    let anchor = create_memo(move || OverlayAnchor::Point(position.get()));
    let (overlay, content) = (NodeRef::new(), NodeRef::new());
    let (toolbar, bar, more_button) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    set_component_state(State {
        overlay,
        content,
        toolbar,
        more: more_button,
    });

    let (requested, requested_touch, requested_position, requested_focusing) =
        (open, touched, position, focusing);
    create_effect(move || {
        let Some(at) = open_at.get() else {
            return;
        };
        requested_position.set(at);
        requested_touch.set(last_pointer().is_some_and(|pointer| pointer.touch));
        requested_focusing.set(open_at_focuses);
        requested.set(true);
    });
    let (pointed, pointed_touch, pointed_position, pointed_focusing) =
        (open, touched, position, focusing);
    let mut pointer_opened = false;
    create_effect(move || {
        if !open_at_pointer.get() {
            if std::mem::take(&mut pointer_opened) {
                pointed.set(false);
            }
            return;
        }
        pointer_opened = true;
        let pointer = last_pointer();
        pointed_position.set(pointer.map_or(Pos2::ZERO, |pointer| pointer.pos));
        pointed_touch.set(pointer.is_some_and(|pointer| pointer.touch));
        pointed_focusing.set(true);
        pointed.set(true);
    });
    let shown = create_memo(move || open.get());
    let touch_shown = create_memo(move || !selection && touched.get());
    let mut ever_open = false;
    let built = create_memo(move || {
        ever_open |= shown.get();
        ever_open
    });
    let toolbar_style = menu.toolbar();
    let has_toolbar = toolbar_style.is_some();
    let toolbar_open = create_memo(move || {
        has_toolbar && toolbar_at.get().is_some() && !open.get() && !disabled.get()
    });
    let toolbar_anchor =
        create_memo(move || OverlayAnchor::Rect(toolbar_at.get().unwrap_or(Rect::NOTHING)));
    let more = Callback::new(move |at: Pos2| {
        position.set(at);
        touched.set(true);
        focusing.set(false);
        open.set(true);
        on_toolbar_close.call();
    });
    create_effect(move || {
        if has_toolbar {
            return;
        }
        if let Some(rect) = toolbar_at.get() {
            more.call(pos2(rect.center().x, rect.bottom()));
        }
    });
    let choose = Callback::new(move |index: usize| {
        on_toolbar_close.call();
        on_select.call(vec![index]);
    });
    let items = items.into_run();
    let toolbar_items = items.clone();
    view! {
        <Interactive
            on_secondary_press={move |press: PointerPress| {
                if disabled.get() || (press.touch && !opens_on_hold) {
                    return;
                }
                position.set(press.pos);
                touched.set(press.touch);
                focusing.set(true);
                open.set(true);
            }}
            on_press_outside={move |pos: Pos2| {
                let on_bar = with_document(|document| {
                    bar.try_get()
                        .and_then(|bar| document.node_rect(bar))
                        .is_some_and(|rect| rect.contains(pos))
                });
                let on_selection = toolbar_at
                    .get_untracked()
                    .is_some_and(|rect| rect.contains(pos));
                if toolbar_open.get_untracked() && !on_bar && !on_selection {
                    on_toolbar_close.call();
                }
            }}
        >
            <List spacing=0.0>
                {children} @sizing={child_size}
                <Show condition={built}>
                    <MenuPopup
                        overlay
                        content
                        items={items.clone()}
                        menu={menu}
                        anchor
                        open={shown}
                        touched={touch_shown}
                        focusing
                        on_dismiss={move || {
                            open.set(false);
                            on_close.call();
                        }}
                        on_select={on_select}
                    />
                </Show>
                <Show condition={toolbar_open}>|| {
                    let (button, panel) = toolbar_style.unwrap_or_else(|| unreachable!());
                    view! {
                        <Toolbar
                            @node_ref=&toolbar
                            items={toolbar_items.clone()}
                            button
                            panel
                            anchor={toolbar_anchor}
                            bar
                            more={more_button}
                            on_choose={choose}
                            on_more={more}
                            on_dismiss={move || on_toolbar_close.call()}
                        />
                    }
                }</Show>
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
    bar: NodeRef,
    more: NodeRef,
    on_choose: Callback<usize>,
    on_more: Callback<Pos2>,
    on_dismiss: ClickCallback,
) -> NodeId {
    let shown = store(RefCell::new(Vec::<Rc<MenuItem>>::new()));
    let buttons = items.build_each(
        shown,
        |_, item| item,
        move |index| {
            let item = shown.with(|shown| shown.borrow()[index].clone());
            match item.children.peek().is_empty() {
                true => view! {
                    <ToolbarButton item index button on_choose />
                },
                false => view! {
                    <List spacing=0.0 />
                },
            }
        },
    );
    view! {
        <Overlay
            anchor
            placement=Placement::Above
            policy=LayerPolicy::Pass
            traps_focus=false
            open=true
            on_dismiss={move |reason: Dismiss| {
                if reason != Dismiss::OutsidePress {
                    on_dismiss.call();
                }
            }}
        >
            {panel.call(view! {
                <List
                    @node_ref=&bar
                    direction=Direction::Horizontal
                    align=Align::Center
                    spacing=0.0
                >
                    {buttons}
                    <unstyled::Button
                        @node_ref=&more
                        label="More"
                        glyph=ICON_MORE_HORIZ
                        tab_stop=false
                        press_focus=false
                        on_click_at={move |press: PointerPress| on_more.call(press.pos)}
                    >|handle: ButtonHandle| {
                        button.call(handle)
                    }</unstyled::Button>
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
    let disabled = item.disabled;
    let action = item.action.clone();
    let clicked = item.on_click;
    view! {
        <unstyled::Button
            label={item.label}
            disabled={item.disabled}
            tab_stop=false
            press_focus=false
            on_click={move || {
                if disabled.get_untracked() {
                    return;
                }
                on_choose.call(index);
                if let Some(action) = &action {
                    action.run();
                }
                clicked.call();
            }}
        >|handle: ButtonHandle| {
            button.call(handle)
        }</unstyled::Button>
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

pub fn context_menu_open(document: &Document, context_menu: NodeId) -> bool {
    document
        .component_state::<State>(context_menu)
        .overlay
        .try_get()
        .is_some_and(|overlay| document.is_overlay_open(overlay))
}

pub fn context_menu_toolbar(document: &Document, context_menu: NodeId) -> Option<NodeId> {
    document
        .component_state::<State>(context_menu)
        .toolbar
        .try_get()
        .filter(|toolbar| document.is_overlay_open(*toolbar))
}

pub fn context_menu_more(document: &Document, context_menu: NodeId) -> NodeId {
    document.component_state::<State>(context_menu).more.get()
}
