use super::*;
use crate::reactive::{bind, create_signal, view};
use crate::styled::Caption;

#[test]
fn an_effect_bound_to_a_node_stops_when_the_node_is_removed() {
    let (label, set_label) = create_signal("one".to_owned());
    let runs = Rc::new(Cell::new(0));
    let counted = runs.clone();
    let (document, [caption]) = toolbar_of(move || {
        let caption = view! {
            <Caption content="Caption" />
        };
        bind(caption, move || {
            label.get();
            counted.set(counted.get() + 1);
        });
        [caption]
    });
    let list = kind_of::<ListNode>(&document, document.root().expect("the toolbar is the root"));

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(runs.get(), 1);

    harness.document_mut().remove_child(list, caption);
    harness.document_mut().remove_node(caption);
    with_installed(harness.document_mut(), |_| set_label.set("two".to_owned()));
    assert_eq!(
        runs.get(),
        1,
        "an effect bound to a node belongs to the node, not to the scope that made it"
    );
}
