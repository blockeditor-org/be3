use beui::reactive::{component, component_size};
use beui::unstyled::MenuItem;

#[component]
fn SizedItem() -> MenuItem {
    let size = component_size(); //~ ERROR is no node, so `component_state`, `component_accessibility`, `component_size`, `component_rect` and `component_placed` have nothing to watch
    let _ = size;
    todo!()
}
