// rustc follows the first error, which says what to write, with others about the same type on
// the same line, so the expected part is the type they share.
use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(count: Signal<u32>) -> NodeId {
    view! { <List spacing=Space::None><Show value={count}><Spacer /></Show></List> } //~ ERROR u32
}
