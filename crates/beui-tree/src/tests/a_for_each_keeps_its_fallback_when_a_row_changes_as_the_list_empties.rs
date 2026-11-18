use super::*;
use crate::reactive::{ForEach, Show};

#[test]
fn a_for_each_keeps_its_fallback_when_a_row_changes_as_the_list_empties() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let keys = create_signal(vec![1_u32]);
    let marked = create_signal(false);
    let page = scope.run(|| {
        view! {
            <Element tag="ul">
                <ForEach
                    value={keys}
                    fallback={|| view! {
                        <Element tag="i" text="empty" />
                    }}
                >|key: u32|
                    <Element tag="b" text={key.to_string()} />
                    <Show value={marked}>
                        <Element tag="i" text="marked" />
                    </Show>
                </ForEach>
            </Element>
        }
    });
    assert!(page.html().contains(">1</b>"), "{}", page.html());
    marked.set(true);
    keys.set(Vec::new());
    let html = page.html();
    assert!(
        html.contains(">empty</i>") && !html.contains("</b>"),
        "a row that changes in the flush that empties the list leaves the fallback shown: {html}"
    );
    marked.set(false);
    let html = page.html();
    assert!(html.contains(">empty</i>"), "{html}");
}
