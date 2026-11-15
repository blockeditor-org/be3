use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate as unstyled;
use crate::button::ButtonHandle;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

pub struct DisclosureHandle {
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub open: Binding<bool>,
}

#[derive(Clone, Copy)]
struct State {
    open: Binding<bool>,
}

#[component]
pub fn Disclosure(
    #[prop(default = ComponentRef::new())] api: ComponentRef<DisclosureApi>,
    spacing: Prop<f32>,
    header: Render<DisclosureHandle, NodeId>,
    children: NodeId,
    open: Binding<bool>,
) -> NodeId {
    api.fill(DisclosureApi(State { open }));

    view! {
        <List spacing=0.0>
            <unstyled::Button
                accessibility={move || {
                    let mut node = Node::new(Role::DisclosureTriangle);
                    node.set_expanded(open.get());
                    node
                }}
                on_click={move || {
                    open.update(|open| *open = !*open);
                }}
            >|handle: ButtonHandle| {
                header.call(DisclosureHandle {
                    hovered: handle.hovered,
                    active: handle.active,
                    focused: handle.focused,
                    open,
                })
            }</unstyled::Button>
            <KeepAlive>
                <Show value={open}>
                    <Kept>
                        <List spacing>
                            <Frame />
                            {children}
                        </List>
                    </Kept>
                </Show>
            </KeepAlive>
        </List>
    }
}

#[derive(Clone, Copy)]
pub struct DisclosureApi(State);

impl DisclosureApi {
    pub fn open(&self) -> bool {
        self.0.open.get()
    }
}
