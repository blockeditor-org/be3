use super::*;
use crate::reactive::{Dynamic, Keyed, ReadSignal};

#[test]
fn dynamic_and_keyed_show_their_fallback_while_the_value_is_none() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let picked = create_signal(None::<(u32, String)>);
    let page = scope.run(|| {
        view! {
            <Element tag="div">
                <Holder tag="dynamic">
                    <Dynamic value={picked} fallback={|| view! {
                        <Element tag="i" text="none" />
                    }}>|(id, name): (u32, String)|
                        <Element tag="p" text={format!("{id}:{name}")} />
                    </Dynamic>
                </Holder>
                <Holder tag="keyed">
                    <Keyed value={picked} fallback={|| view! {
                        <Element tag="i" text="none" />
                    }} key={|(id, _): (u32, String)| id}>|row: ReadSignal<(u32, String)>|
                        <Element tag="p" text={create_memo(move || row.get().1)} />
                    </Keyed>
                </Holder>
            </Element>
        }
    });
    let html = page.html();
    assert_eq!(html.matches("none").count(), 2, "{html}");

    picked.set(Some((1, "one".to_owned())));
    let html = page.html();
    assert!(html.contains(">1:one</p></dynamic>"), "{html}");
    assert!(html.contains("<keyed id=2><p id=6>one</p></keyed>"), "{html}");

    picked.set(Some((1, "uno".to_owned())));
    let html = page.html();
    assert!(html.contains(">1:uno</p></dynamic>"), "{html}");
    assert!(
        html.contains("<keyed id=2><p id=6>uno</p></keyed>"),
        "a `Keyed` whose key holds updates what it built in place: {html}"
    );

    picked.set(None);
    let html = page.html();
    assert_eq!(html.matches("none").count(), 2, "{html}");
}
