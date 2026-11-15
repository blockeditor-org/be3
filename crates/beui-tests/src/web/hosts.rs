//! Host nodes on the HTML core: what the server renders for control flow in a slot that takes
//! one node is claimed when the page hydrates, an empty host included, and then swaps.

use beui::primitives::*;
use beui::reactive::*;

use super::live::{Checked, every_node, expect, same};

#[component]
fn Hosted(shown: Signal<bool>) -> NodeId {
    view! {
        <List spacing=4.0>
            <Frame width=80.0>
                <Show
                    value={shown}
                    fallback={|| view! {
                        <Text>"hidden"</Text>
                    }}
                >
                    <Text>"shown"</Text>
                </Show>
            </Frame>
            <Interactive>
                <Show value={shown}>
                    <Text>"badge"</Text>
                </Show>
            </Interactive>
        </List>
    }
}

pub(super) fn hydrating_claims_host_nodes() -> Checked {
    let _root = RootScope::new();
    let shown = create_signal(false);
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or("checks run in a page")?;
    let container = document.create_element("div").map_err(|_| "a div")?;
    container.set_id("hosts");
    document
        .body()
        .ok_or("the page has a body")?
        .append_child(&container)
        .map_err(|_| "the container joins the page")?;
    container.set_inner_html(&beui::html::render_to_string(move || {
        view! {
            <Root clear_color={beui::Color32::WHITE}>
                <Hosted shown />
            </Root>
        }
    }));
    let served = container.inner_html();
    expect(served.matches("display: contents").count() == 2, || {
        format!("the server did not render both hosts: {served}")
    })?;
    let before = every_node(&container);
    beui::html::hydrate("hosts", move || {
        view! {
            <Root clear_color={beui::Color32::WHITE}>
                <Hosted shown />
            </Root>
        }
    })?;
    let after = every_node(&container);
    expect(
        before.len() == after.len() && before.iter().zip(&after).all(|(a, b)| same(a, b)),
        || {
            format!(
                "hydration replaced host nodes: {} before, {} after: {}",
                before.len(),
                after.len(),
                container.inner_html()
            )
        },
    )?;
    shown.set(true);
    flush();
    let html = container.inner_html();
    expect(
        html.contains("shown") && html.contains("badge") && !html.contains("hidden"),
        || format!("the hydrated hosts did not swap: {html}"),
    )
}
