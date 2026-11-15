use beui::reactive::*;

#[component]
fn Case(
    #[prop(required)] label: Prop<String>, //~ ERROR `#[prop(required)]` is for a `Binding<T>` with no default
) -> beui::NodeId {
    view! { <Text>{label}</Text> }
}
