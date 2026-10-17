use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn disposing_a_scope_removes_the_nodes_it_owns() {
    let _root = crate::reactive::RootScope::new();
    let (cleaned, set_cleaned) = create_signal::<Vec<&'static str>>(Vec::new());
    let (label, set_label) = create_signal("one".to_owned());
    let (shown, set_shown) = create_signal(true);
    let text = NodeRef::new();
    let document = build({
        move || {
            view! {
                <List spacing=0.0>
                    <Show condition={shown}>|| {
                        component("Outer", move || {
                            on_cleanup(move || set_cleaned.update(|cleaned| cleaned.push("outer")));
                            let inner = component("Inner", move || {
                                on_cleanup(move || set_cleaned.update(|cleaned| cleaned.push("inner")));
                                view! {
                                    <Text @node_ref=&text string={label} />
                                }
                            });
                            view! {
                                <Frame>{inner}</Frame>
                            }
                        })
                    }</Show>
                </List>
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let built = text.get();
    let count = harness.document().arena.len();
    assert!(cleaned.with_untracked(|cleaned| cleaned.is_empty()));

    with_installed(harness.document_mut(), |_| set_shown.set(false));
    assert_eq!(cleaned.get_untracked(), ["inner", "outer"]);
    assert!(
        !harness.document().contains(built),
        "a node goes with the scope that built it"
    );
    assert_eq!(
        harness.document().arena.len(),
        count - 2,
        "the frame and the text both go, and nothing else"
    );

    with_installed(harness.document_mut(), |_| set_label.set("two".to_owned()));
    harness.frame(Vec::new());
}
