use accesskit::{Node, Role};
use beui_view::reactive::NodeId;
use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::*;

#[component]
pub fn Table(#[prop(default = String::new())] label: Prop<String>, children: Child) -> NodeId {
    component_accessibility(labelled(Role::Table, label));
    view! {
        <List spacing=0.0>{children}</List>
    }
}

#[component]
pub fn TableRow(children: Child) -> NodeId {
    component_accessibility(Node::new(Role::Row));
    view! {
        <List spacing=0.0>{children}</List>
    }
}

#[component]
pub fn TableColumnHeader(label: Prop<String>, children: Child) -> NodeId {
    component_accessibility(labelled(Role::ColumnHeader, label));
    view! {
        <List spacing=0.0>{children}</List>
    }
}

#[component]
pub fn TableRowHeader(label: Prop<String>, children: Child) -> NodeId {
    component_accessibility(labelled(Role::RowHeader, label));
    view! {
        <List spacing=0.0>{children}</List>
    }
}

#[component]
pub fn TableCell(label: Prop<String>, children: Child) -> NodeId {
    component_accessibility(labelled(Role::Cell, label));
    view! {
        <List spacing=0.0>{children}</List>
    }
}

fn labelled(role: Role, label: Prop<String>) -> Memo<Node> {
    create_memo(move || {
        let mut node = Node::new(role);
        let label = label.get();
        if !label.is_empty() {
            node.set_label(label);
        }
        node
    })
}
