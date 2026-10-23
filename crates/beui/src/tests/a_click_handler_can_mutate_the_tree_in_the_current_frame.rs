use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_click_handler_can_mutate_the_tree_in_the_current_frame() {
    let _root = crate::reactive::RootScope::new();
    let button = NodeRef::new();
    let document = build_node(move || {
        view! {
            <LabelledButton
                @node_ref=&button
                label="replace"
                on_click={|| with_document(|document| {
                    let replacement = view! {
                        <Frame fill=Color32::BLACK radius=0.0 />
                    };
                    document.set_root(replacement);
                })}
            />
        }
    });
    let button = button.get();
    let mut harness = Harness::new(document);
    harness.frame(vec![]);
    harness.click(harness.center(button));
    assert_ne!(harness.document().root(), Some(button));
    assert!(harness.document().node_rect(button).is_none());
    assert_eq!(harness.frame(vec![]).main().shapes().len(), 1);
}
