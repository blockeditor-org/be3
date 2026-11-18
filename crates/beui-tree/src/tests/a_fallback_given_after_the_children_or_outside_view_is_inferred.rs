use super::*;
use crate::reactive::SingleChild;

#[test]
fn a_fallback_given_after_the_children_or_outside_view_is_inferred() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let shown = create_signal(false);
    let page = scope.run(|| {
        let after = Show()
            .value(shown)
            .then(|_| {
                SingleChild(view! {
                    <Element tag="b" text="then" />
                })
            })
            .fallback(|| {
                view! {
                    <Element tag="i" text="else" />
                }
            })
            .build();
        let before = Show()
            .fallback(|| {
                view! {
                    <Element tag="i" text="else" />
                }
            })
            .value(shown)
            .then(|_| {
                view! {
                    <Element tag="b" text="then" />
                }
            })
            .build();
        view! {
            <Element tag="section">
                <Holder tag="div">{after}</Holder>
                <Element tag="div">{before}</Element>
            </Element>
        }
    });
    let html = page.html();
    assert_eq!(html.matches("else").count(), 2, "{html}");
    shown.set(true);
    let html = page.html();
    assert_eq!(html.matches("then").count(), 2, "{html}");
}
