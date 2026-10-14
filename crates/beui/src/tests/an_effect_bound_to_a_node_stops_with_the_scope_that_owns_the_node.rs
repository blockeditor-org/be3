use super::*;
use crate::reactive::{Scope, bind, create_signal, view};
use crate::styled::Caption;

#[test]
fn an_effect_bound_to_a_node_stops_with_the_scope_that_owns_the_node() {
    let _root = crate::reactive::RootScope::new();
    let (label, set_label) = create_signal("one".to_owned());
    let (runs, set_runs) = create_signal(0);
    let (_, set_owner) = create_signal::<Option<Scope>>(None);
    let (document, [caption]) = toolbar_of(move || {
        let scope = Scope::new();
        let caption = scope.run(|| {
            view! {
                <Caption content="Caption" />
            }
        });
        set_owner.update(|owner| *owner = Some(scope));
        bind(caption, move || {
            label.get();
            set_runs.update(|runs| *runs += 1);
        });
        [caption]
    });
    let list = kind_of::<ListNode>(&document, document.root().expect("the toolbar is the root"));

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(runs.get_untracked(), 1);

    harness.document_mut().remove_child(list, caption);
    with_installed(harness.document_mut(), |_| {
        drop(set_owner.update(std::mem::take))
    });
    assert!(!harness.document().contains(caption));
    with_installed(harness.document_mut(), |_| set_label.set("two".to_owned()));
    assert_eq!(
        runs.get_untracked(),
        1,
        "an effect bound to a node belongs to the scope that owns the node, not to the one that bound it"
    );
}
