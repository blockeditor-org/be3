use beui_html::{elements as html, *};

#[component]
fn Case() -> Node {
    view! { <html::Button href="/" /> } //~ ERROR no method named `href`
}
