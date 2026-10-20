use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate as unstyled;
use crate::button::ButtonHandle;
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

pub struct DisclosureHandle {
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub open: ReadSignal<bool>,
}

struct State {
    open: ReadSignal<bool>,
}

#[component]
pub fn Disclosure(
    spacing: Prop<f32>,
    header: Render<DisclosureHandle>,
    children: Child,
    open: Prop<bool>,
    on_toggle: Callback<bool>,
) -> NodeId {
    let current = create_signal(open.get_untracked());
    create_effect(move || current.set(open.get()));

    set_component_state(State {
        open: current.read_only(),
    });

    let accessibility = create_memo(move || {
        let mut node = Node::new(Role::DisclosureTriangle);
        node.set_expanded(current.get());
        node
    });

    view! {
        <List spacing=0.0>
            <unstyled::Button
                accessibility
                on_click={move || {
                    let next = !current.get_untracked();
                    current.set(next);
                    on_toggle.call(next);
                }}
            >|handle: ButtonHandle| {
                header.call(DisclosureHandle {
                    hovered: handle.hovered,
                    active: handle.active,
                    focused: handle.focused,
                    open: current.read_only(),
                })
            }</unstyled::Button>
            <Frame visible={current}>
                <List spacing>
                    <Frame />
                    {children}
                </List>
            </Frame>
        </List>
    }
}

pub fn disclosure_open(document: &Document, disclosure: NodeId) -> bool {
    document.component_state::<State>(disclosure).open.get()
}
