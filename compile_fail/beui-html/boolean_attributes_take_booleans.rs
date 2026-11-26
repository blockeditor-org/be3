use beui_html::{elements as html, *};

#[component]
fn Case() -> Node {
    view! { <html::Button formnovalidate="true" /> } //~ ERROR cannot be passed as a `bool` prop
}
