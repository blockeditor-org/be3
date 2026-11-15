use super::*;
use crate::reactive::{Case, Match};

#[test]
fn a_match_shows_the_first_case_that_holds() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let count = create_signal(0_u32);
    let page = scope.run(|| {
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
                    <Case when={|count: &u32| *count < 3}>|count: u32|
                        <Element tag="p" text={format!("a few: {count}")} />
                    </Case>
                </Match>
            </Element>
        }
    });
    assert!(page.html().contains(">none</p>"), "{}", page.html());

    count.set(2);
    assert!(page.html().contains(">a few: 2</p>"), "{}", page.html());

    count.set(7);
    let html = page.html();
    assert!(html.contains(">many</i>") && !html.contains("</p>"), "{html}");
}
