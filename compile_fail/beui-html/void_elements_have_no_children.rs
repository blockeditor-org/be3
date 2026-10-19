use beui_html::{elements as html, *};

#[component]
fn Case() -> Node {
    view! { <html::Input>"child"</html::Input> } //~ ERROR no method named `children_block`
}
