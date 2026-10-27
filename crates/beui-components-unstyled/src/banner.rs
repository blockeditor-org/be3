use accesskit::{Live, Node, Role};
use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

#[component]
pub fn Banner(label: Prop<String>, children: Child) -> NodeId {
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Alert);
        node.set_live(Live::Assertive);
        let label = label.get();
        if !label.is_empty() {
            node.set_label(label);
        }
        node
    }));
    view! {
        <List spacing=0.0>{children}</List>
    }
}
