use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate as unstyled;
use crate::button::{ButtonHandle, ButtonState};
use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

pub struct DisclosureHandle {
    pub state: Prop<ButtonState>,
    pub open: Binding<bool>,
}

struct State {
    open: Binding<bool>,
}

#[component]
pub fn Disclosure(
    spacing: Prop<f32>,
    header: Render<DisclosureHandle>,
    children: Child,
    open: Binding<bool>,
) -> NodeId {
    set_component_state(State { open });

    let accessibility = create_memo(move || {
        let mut node = Node::new(Role::DisclosureTriangle);
        node.set_expanded(open.get());
        node
    });

    view! {
        <List spacing=0.0>
            <unstyled::Button
                accessibility
                on_click={move || {
                    open.update(|open| *open = !*open);
                }}
            >|handle: ButtonHandle| {
                header.call(DisclosureHandle {
                    state: handle.state,
                    open,
                })
            }</unstyled::Button>
            <Frame visible={open}>
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
