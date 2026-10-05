use std::rc::Rc;

use accesskit::{Node, Role};
use beui_view::reactive::Prop;

pub fn labelled_node(
    role: Role,
    accessibility: Option<Prop<Node>>,
    label: Prop<String>,
) -> Prop<Node> {
    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(role)));
    Prop::Dynamic(Rc::new(move || {
        let mut node = accessibility.get();
        let label = label.get();
        if node.label().is_none() && !label.is_empty() {
            node.set_label(label);
        }
        node
    }))
}
