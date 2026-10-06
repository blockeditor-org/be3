use super::*;

#[test]
fn a_mock_dom_follows_its_signals_through_show_and_for_each() {
    let scope = Scope::new();
    let (title, set_title) = create_signal("Groceries".to_string());
    let (items, set_items) = create_signal(vec!["milk".to_string(), "eggs".to_string()]);
    let page = scope.run(|| {
        let empty = create_memo({
            let items = items.clone();
            move || items.get().is_empty()
        });
        view! {
            <Element tag="div">
                <Element tag="h1" text={title} />
                <Show condition={empty}>
                    <Element tag="p" text="nothing to buy" />
                </Show>
                <Element tag="ul">
                    <ForEach keys={items}>
                        {|item: String| view! {
                            <Element tag="li" text={item} />
                        }}
                    </ForEach>
                </Element>
            </Element>
        }
    });

    assert_eq!(
        page.html(),
        "<div><h1>Groceries</h1><ul><li>milk</li><li>eggs</li></ul></div>"
    );

    set_items.set(vec!["eggs".to_string(), "bread".to_string()]);
    assert_eq!(
        page.html(),
        "<div><h1>Groceries</h1><ul><li>eggs</li><li>bread</li></ul></div>",
        "a keyed list follows its keys in a tree that is no beui document"
    );

    set_items.set(Vec::new());
    set_title.set("Pantry".to_string());
    assert_eq!(
        page.html(),
        "<div><h1>Pantry</h1><p>nothing to buy</p><ul></ul></div>",
        "a `Show` fills its place among its siblings when its condition turns true"
    );
}
