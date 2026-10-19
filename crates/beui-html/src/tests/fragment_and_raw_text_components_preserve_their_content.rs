use super::*;

#[test]
fn fragment_and_raw_text_components_preserve_their_content() {
    let html = render_fragment(|| {
        let nodes: Children<Node> = [view! {
            <Element tag="p">"before"</Element>
        }]
        .into_iter()
        .collect();
        view! {
            {nodes}
            <Stylesheet css="p > b { color: red; }" />
            <Script kind="module" source="if (1 < 2) console.log('ok');" />
            <Element tag="p">"after"</Element>
        }
    });
    assert_eq!(
        html,
        "<p>before</p><style>p > b { color: red; }</style><script type=\"module\">if (1 < 2) console.log('ok');</script><p>after</p>"
    );
}
