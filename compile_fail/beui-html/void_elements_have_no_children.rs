use beui_html::{elements as html, *};

#[component]
fn Case() -> Node {
    view! { <html::Input>"child"</html::Input> } //~ ERROR this component takes no children
}
