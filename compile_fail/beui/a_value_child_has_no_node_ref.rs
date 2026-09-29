use beui::reactive::{NodeRef, view};
use beui::unstyled::MenuItem;

fn case(node: NodeRef) -> MenuItem {
    view! { <MenuItem @node_ref=&node label="Copy" /> } //~ ERROR is no node, so `@test_id` and `@node_ref` have nothing to name
}
