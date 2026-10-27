use super::*;

#[test]
fn fragments_group_children_without_adding_html_nodes() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let shown = create_signal(true);
    let node = scope.run(|| {
        view! {
            <elements::Div>
                <></>
                <>
                    "Before"
                    <Show condition={shown}>
                        <>
                            <elements::Span>"One"</elements::Span>
                            <>
                                <elements::Span>"Two"</elements::Span>
                            </>
                        </>
                    </Show>
                    "After"
                </>
            </elements::Div>
        }
    });
    assert_eq!(
        node.to_html(),
        "<div>Before<!--[--><span>One</span><span>Two</span><!--]-->After</div>"
    );
    shown.set(false);
    assert_eq!(node.to_html(), "<div>Before<!--[--><!--]-->After</div>");
    assert_eq!(
        render_fragment(|| view! {
            <>
                <elements::P>"Root"</elements::P>
                <></>
            </>
        }),
        "<p>Root</p>"
    );
    assert_eq!(
        render_fragment(|| view! {
            <></>
        }),
        ""
    );
}
