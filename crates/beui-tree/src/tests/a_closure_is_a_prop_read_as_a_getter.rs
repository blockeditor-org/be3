use super::*;

#[test]
fn a_closure_is_a_prop_read_as_a_getter() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let hex = create_signal(false);
    let count = create_signal(2);
    let page = scope.run(|| {
        view! {
            <Element tag="div">
                <Show value={move || !hex.get()}>
                    <Element tag="p" text={move || format!("{} lines", count.get())} />
                </Show>
            </Element>
        }
    });
    assert_eq!(page.html(), "<div id=0><p id=1>2 lines</p></div>");

    count.set(3);
    assert_eq!(page.html(), "<div id=0><p id=1>3 lines</p></div>");

    hex.set(true);
    assert_eq!(page.html(), "<div id=0></div>");
}
