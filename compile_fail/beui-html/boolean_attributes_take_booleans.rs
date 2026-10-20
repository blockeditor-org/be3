use beui_html::{elements as html, *};

#[component]
fn Case() -> Node {
    view! { <html::Button formnovalidate="true" /> } //~ ERROR IntoProp<bool>
}
