use super::*;

#[test]
fn a_mock_dom_follows_its_signals_through_show_and_for_each() {
    fresh_ids();
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
        "<div id=2><h1 id=0>Groceries</h1><ul id=1><li id=3>milk</li><li id=4>eggs</li></ul></div>"
    );

    set_items.set(vec!["eggs".to_string(), "bread".to_string()]);
    assert_eq!(
        page.html(),
        "<div id=2><h1 id=0>Groceries</h1><ul id=1><li id=4>eggs</li><li id=5>bread</li></ul></div>",
        "a key that stays keeps the element built for it, and only a new key builds one"
    );

    set_items.set(Vec::new());
    set_title.set("Pantry".to_string());
    assert_eq!(
        page.html(),
        "<div id=2><h1 id=0>Pantry</h1><p id=6>nothing to buy</p><ul id=1></ul></div>",
        "a changed text updates its element in place, and a `Show` fills its place among its siblings"
    );

    set_items.set(vec!["eggs".to_string()]);
    assert_eq!(
        page.html(),
        "<div id=2><h1 id=0>Pantry</h1><ul id=1><li id=7>eggs</li></ul></div>",
        "a key that left and came back is built again, and nothing else is"
    );
}
