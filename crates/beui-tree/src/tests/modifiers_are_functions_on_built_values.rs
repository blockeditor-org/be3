use super::*;

#[test]
fn modifiers_are_functions_on_built_values() {
    let _root = ::reactive::RootScope::new();
    fresh_ids();
    let page = view! {
        <Element tag="div">
            <Element @modifiers::suffix="!" tag="p" text="hello" @modifiers::suffix="?" />
            {view! {
                <Element tag="span" text="value" />
            }} @modifiers::suffix="."
        </Element>
    };
    assert_eq!(
        page.html(),
        "<div id=2><p id=0>hello!?</p><span id=1>value.</span></div>"
    );
    let html = view! {
        <Element tag="p" text="done" @modifiers::stringify="!" />
    };
    assert_eq!(html, "<p id=3>done</p>!");
    let append = |text: &str, suffix: &str| format!("{text}{suffix}");
    assert_eq!(
        view! {
            "literal" @append="!"
        },
        "literal!"
    );
}
