use super::*;

#[test]
fn dynamic_ranges_are_marked_for_hydration() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let shown = create_signal(false);
    let items = create_signal(vec![1, 2]);
    let root = Node::new(Kind::Fragment);
    scope.run(|| {
        let children: Children<Node> = view! {
            "head"
            <Show condition={shown}>"shown"</Show>
            <elements::Ul>
                <elements::Li>"static"</elements::Li>
                <ForEach keys={items}>|item: i32|
                    <elements::Li>{item.to_string()}</elements::Li>
                </ForEach>
            </elements::Ul>
        };
        children.mount(&root);
    });
    assert_eq!(
        root.to_html(),
        "head<!--[--><!--]--><ul><li>static</li><!--[--><li>1</li><li>2</li><!--]--></ul>"
    );
    shown.set(true);
    items.set(vec![]);
    assert_eq!(
        root.to_html(),
        "head<!--[-->shown<!--]--><ul><li>static</li><!--[--><!--]--></ul>"
    );
}
