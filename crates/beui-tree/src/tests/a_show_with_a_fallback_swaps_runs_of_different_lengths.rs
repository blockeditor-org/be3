use super::*;

#[test]
fn a_show_with_a_fallback_swaps_runs_of_different_lengths() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let editing = create_signal(false);
    let page = scope.run(|| {
        view! {
            <Element tag="div">
                <Element tag="b" text="before" />
                <Show
                    value={editing}
                    fallback={|| view! {
                        <Element tag="i" text="one" />
                        <Element tag="i" text="two" />
                        <Element tag="i" text="three" />
                    }}
                >||
                    <Element tag="p" text="first" />
                    <Element tag="p" text="second" />
                </Show>
                <Element tag="b" text="after" />
            </Element>
        }
    });
    let shown = || {
        let html = page.html();
        let texts: Vec<_> = ["before", "one", "two", "three", "first", "second", "after"]
            .into_iter()
            .filter(|text| html.contains(text))
            .collect();
        (page.0.children.borrow().len(), texts)
    };
    assert_eq!(
        shown(),
        (5, vec!["before", "one", "two", "three", "after"]),
        "while the condition is false the fallback's three children show"
    );

    editing.set(true);
    flush();
    assert_eq!(shown(), (4, vec!["before", "first", "second", "after"]));

    editing.set(false);
    flush();
    assert_eq!(shown(), (5, vec!["before", "one", "two", "three", "after"]));
}
