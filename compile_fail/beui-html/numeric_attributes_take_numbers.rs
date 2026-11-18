use beui_html::{elements as html, *};

#[component]
fn Case() -> Node {
    view! { <html::Input maxlength="200" /> } //~ ERROR cannot be passed as a `Option<u32>` prop
}
