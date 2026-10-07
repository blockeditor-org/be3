use beui_macros::{component, view};

use crate::menu::MenuItem;
use crate::menu_popup::{MenuPopup, MenuStyle};
use beui_core::base::overlay::OverlayAnchor;
use beui_core::document::Document;
use beui_core::geometry::Pos2;
use beui_core::input::PointerPress;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, Child, Children, ClickCallback, Interactive, ItemSize, List, NodeRef, Prop, clone,
    create_effect, create_memo, create_signal, last_pointer, set_component_state,
};

struct State {
    overlay: NodeRef,
    content: NodeRef,
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
    on_close: ClickCallback,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let (open, set_open) = create_signal(false);
    let (touched, set_touched) = create_signal(false);
    let (focusing, set_focusing) = create_signal(true);
    let (position, set_position) = create_signal(Pos2::ZERO);
    let anchor = create_memo(move || OverlayAnchor::Point(position.get()));
    let (overlay, content) = (NodeRef::new(), NodeRef::new());
    set_component_state(State {
        overlay: overlay.clone(),
        content: content.clone(),
    });

    let (requested, requested_touch, requested_position, requested_focusing) = (
        set_open.clone(),
        set_touched.clone(),
        set_position.clone(),
        set_focusing.clone(),
    );
    create_effect(move || {
        let Some(at) = open_at.get() else {
            return;
        };
        requested_position.set(at);
        requested_touch.set(last_pointer().is_some_and(|pointer| pointer.touch));
        requested_focusing.set(open_at_focuses);
        requested.set(true);
    });
    let (pointed, pointed_touch, pointed_position, pointed_focusing) = (
        set_open.clone(),
        set_touched.clone(),
        set_position.clone(),
        set_focusing.clone(),
    );
    create_effect(move || {
        if !open_at_pointer.get() {
            return;
        }
        let pointer = last_pointer();
        pointed_position.set(pointer.map_or(Pos2::ZERO, |pointer| pointer.pos));
        pointed_touch.set(pointer.is_some_and(|pointer| pointer.touch));
        pointed_focusing.set(true);
        pointed.set(true);
    });
    let shown = create_memo(clone!(open -> move || open.get()));
    let touched = create_memo(move || !selection && touched.get());
    view! {
        <Interactive
            on_secondary_press={clone!(set_open -> move |press: PointerPress| {
                if disabled.get() {
                    return;
                }
                set_position.set(press.pos);
                set_touched.set(press.touch);
                set_focusing.set(true);
                set_open.set(true);
            })}
        >
            <List spacing=0.0>
                {children} @sizing={child_size}
                <MenuPopup
                    overlay
                    content
                    items={items.into_run()}
                    menu
                    anchor
                    open={shown}
                    touched
                    focusing
                    on_dismiss={move || {
                        set_open.set(false);
                        on_close.call();
                    }}
                    on_select={move |path: Vec<usize>| on_select.call(path)}
                />
            </List>
        </Interactive>
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
