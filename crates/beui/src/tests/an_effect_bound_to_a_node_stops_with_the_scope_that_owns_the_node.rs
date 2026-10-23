use super::*;
use crate::reactive::*;
use crate::styled::Text;
use crate::styled::*;

#[test]
fn an_effect_bound_to_a_node_stops_with_the_scope_that_owns_the_node() {
    let _root = crate::reactive::RootScope::new();
    let label = create_signal("one".to_owned());
    let runs = create_signal(0);
    let owner = create_signal::<Option<Scope>>(None);
    let (document, [caption]) = toolbar_of(move || {
        let scope = Scope::new();
        let caption = scope.run(|| {
            view! {
                <Text style=TextStyle::CAPTION>"Caption"</Text>
            }
        });
        owner.update(|owner| *owner = Some(scope));
        bind(caption, move || {
            label.get();
            runs.update(|runs| *runs += 1);
        });
        [caption]
    });
    let list = kind_of::<ListNode>(&document, document.root().expect("the toolbar is the root"));

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(runs.get_untracked(), 1);

    harness.document_mut().remove_child(list, caption);
    with_installed(harness.document_mut(), |_| {
        drop(owner.update(std::mem::take))
    });
    assert!(!harness.document().contains(caption));
    with_installed(harness.document_mut(), |_| label.set("two".to_owned()));
    assert_eq!(
        runs.get_untracked(),
        1,
        "an effect bound to a node belongs to the scope that owns the node, not to the one that bound it"
    );
}
