use super::*;
use crate::reactive::ForEach;

#[test]
fn a_for_each_shows_its_fallback_while_it_has_no_keys() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let keys = create_signal(Vec::<u32>::new());
    let page = scope.run(|| {
        view! {
            <Element tag="ul">
                <ForEach
                    value={keys}
                    fallback={|| view! {
                        <Element tag="i" text="empty" />
                    }}
                >|key: u32|
                    <Element tag="li" text={key.to_string()} />
                </ForEach>
            </Element>
        }
    });
    let html = page.html();
    assert!(html.contains(">empty</i>"), "{html}");

    keys.set(vec![1, 2]);
    let html = page.html();
    assert!(!html.contains("empty"), "{html}");
    assert!(
        html.contains(">1</li>") && html.contains(">2</li>"),
        "{html}"
    );

    keys.set(Vec::new());
    let html = page.html();
    assert!(html.contains(">empty</i>"), "{html}");
    assert!(!html.contains("</li>"), "{html}");
}
