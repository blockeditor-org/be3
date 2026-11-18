use beui::reactive::*;

#[component]
fn Case(
    #[prop(required, default = 0)] count: Binding<u32>, //~ ERROR `#[prop(required)]` is for a prop with no default
) -> beui::NodeId {
    view! { <Text>{move || count.get().to_string()}</Text> }
}
