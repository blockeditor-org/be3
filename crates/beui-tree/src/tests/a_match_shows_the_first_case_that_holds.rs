use super::*;
use crate::reactive::{Case, Match, ReadSignal};

#[test]
fn a_match_shows_the_first_case_that_holds() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let count = create_signal(0_u32);
    let built = std::rc::Rc::new(std::cell::Cell::new(0));
    let page = scope.run(|| {
        let built = built.clone();
        view! {
            <Element tag="div">
                <Match
                    value={count}
                    fallback={|| view! {
                        <Element tag="i" text="many" />
                    }}
                >
                    <Case is={0}>
                        <Element tag="p" text="none" />
                    </Case>
                    <Case when={|count: &u32| *count < 3}>|count: ReadSignal<u32>| {
                        built.set(built.get() + 1);
                        view! {
                            <Element
                                tag="p"
                                text={create_memo(move || format!("a few: {}", count.get()))}
                            />
                        }
                    }</Case>
                </Match>
            </Element>
        }
    });
    assert!(page.html().contains(">none</p>"), "{}", page.html());

    count.set(1);
    assert!(page.html().contains(">a few: 1</p>"), "{}", page.html());
    count.set(2);
    assert!(page.html().contains(">a few: 2</p>"), "{}", page.html());
    assert_eq!(
        built.get(),
        1,
        "a case is kept while it stays the one that holds"
    );

    count.set(7);
    let html = page.html();
    assert!(
        html.contains(">many</i>") && !html.contains("</p>"),
        "{html}"
    );

    let picked = create_signal(None::<u8>);
    let page = scope.run(|| {
        view! {
            <Element tag="div">
                <Match value={picked}>
                    <Case is={None}>
                        <Element tag="i" text="nothing" />
                    </Case>
                    <Case is={Some(1)}>
                        <Element tag="p" text="one" />
                    </Case>
                </Match>
            </Element>
        }
    });
    assert!(page.html().contains(">nothing</i>"), "{}", page.html());
    picked.set(Some(1));
    assert!(
        page.html().contains(">one</p>"),
        "a `Match` over an `Option` compares with `Some(..)`: {}",
        page.html()
    );
}
