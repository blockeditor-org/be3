use beui::reactive::*;
use beui::unstyled::MenuItem;

fn case(node: NodeRef) -> MenuItem {
    view! { <MenuItem @node_ref=&node label="Copy" /> } //~ ERROR is no node, so `@node_ref` has nothing to name
}
