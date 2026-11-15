use beui::reactive::*;

#[component]
fn Case(
    #[prop(required)] label: Prop<String>, //~ ERROR `#[prop(required)]` is for a prop whose type has a value when it is left out
) -> beui::NodeId {
    view! { <Text>{label}</Text> }
}
