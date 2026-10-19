use super::*;

#[component]
fn Panel(before: Children<Node>, children: Children<Node>) -> Node {
    view! {
        <elements::Section>{before}{children}</elements::Section>
    }
}

#[test]
fn component_valued_props_accept_single_empty_and_composed_views() {
    let output = render_fragment(|| {
        view! {
            <Panel before=<elements::H1>"Title"</elements::H1>>
                <Panel before=<></>>"Empty"</Panel>
                <Panel
                    before=<>
                        "One"
                        <elements::Span>"Two"</elements::Span>
                    </>
                >
                    "Three"
                </Panel>
            </Panel>
        }
    });
    assert_eq!(
        output,
        "<section><h1>Title</h1><section>Empty</section><section>One<span>Two</span>Three</section></section>"
    );
}
