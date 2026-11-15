use super::*;

#[test]
fn a_kept_alive_show_whose_condition_reruns_unchanged_leaves_its_slot_alone() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let count = create_signal(1);
    let page = scope.run(|| {
        view! {
            <Element tag="div">
                <Show keep_alive=true condition={move || count.get() > 0} fallback={|| view! {
                    <Element tag="i" text="none" />
                }}>
                    <Element tag="input" text="focused field" />
                </Show>
            </Element>
        }
    });
    flush();
    let filled = page.0.children.borrow().revision();
    for next in 2..6 {
        count.set(next);
        flush();
    }
    assert_eq!(
        page.0.children.borrow().revision(),
        filled,
        "a condition that reruns to the same value hides and refills nothing, so a focused field keeps its focus"
    );
    count.set(0);
    flush();
    assert!(page.html().contains("none"));
}
