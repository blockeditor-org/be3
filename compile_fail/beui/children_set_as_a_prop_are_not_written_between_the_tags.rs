use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case() -> NodeId {
    view! {
        <List spacing=Space::None>
            <Loading fallback={|_: Pending| view! { <Spacer /> }} content={|| view! { <Spacer /> }}><Spacer /></Loading> //~ ERROR `content` is set both as a prop and as this tag's children
        </List>
    }
}
