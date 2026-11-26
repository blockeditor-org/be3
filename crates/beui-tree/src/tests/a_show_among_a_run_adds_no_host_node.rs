use super::*;

#[test]
fn a_show_among_a_run_adds_no_host_node() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let shown = create_signal(false);
    let page = scope.run(|| {
        view! {
            <Element tag="div">
                <Show
                    value={shown}
                    fallback={|| view! {
                        <Element tag="i" text="hidden" />
                    }}
                >
                    <Element tag="b" text="shown" />
                </Show>
            </Element>
        }
    });
    assert_eq!(page.html(), "<div id=0><i id=1>hidden</i></div>");
    shown.set(true);
    assert_eq!(
        page.html(),
        "<div id=0><b id=2>shown</b></div>",
        "among a run the control flow fills its place with no node of its own"
    );
}
