use super::*;

#[test]
fn view_control_flow_and_bindings_update_the_html_tree() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let value = create_signal("first".to_string());
    let shown = create_signal(true);
    let items = create_signal(vec!["one".to_string(), "two".to_string()]);
    let node = scope.run(|| {
        view! {
            <Element tag="main" @attr={("title", value)}>
                <Text value={value} />
                <Show condition={shown}>
                    <Element tag="p">"visible"</Element>
                </Show>
                <Element tag="ul">
                    <ForEach keys={items}>|item: String|
                        <Element tag="li">{item}</Element>
                    </ForEach>
                </Element>
            </Element>
        }
    });
    assert_eq!(
        node.to_html(),
        "<main title=\"first\">first<!--[--><p>visible</p><!--]--><ul><!--[--><li>one</li><li>two</li><!--]--></ul></main>"
    );
    value.set("<&".into());
    shown.set(false);
    items.set(vec!["two".into(), "three".into()]);
    assert_eq!(
        node.to_html(),
        "<main title=\"&lt;&amp;\">&lt;&amp;<!--[--><!--]--><ul><!--[--><li>two</li><li>three</li><!--]--></ul></main>"
    );
}
