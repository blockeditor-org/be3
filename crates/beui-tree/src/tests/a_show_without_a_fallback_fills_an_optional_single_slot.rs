use super::*;

#[test]
fn a_show_without_a_fallback_fills_an_optional_single_slot() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let shown = create_signal(false);
    let page = scope.run(|| {
        view! {
            <MaybeHolder tag="div">
                <Show value={shown}>
                    <Element tag="p" text="badge" />
                </Show>
            </MaybeHolder>
        }
    });
    assert_eq!(page.html(), "<div id=1><host id=0></host></div>");

    shown.set(true);
    flush();
    assert_eq!(
        page.html(),
        "<div id=1><host id=0><p id=2>badge</p></host></div>"
    );

    shown.set(false);
    flush();
    assert_eq!(page.html(), "<div id=1><host id=0></host></div>");
}
