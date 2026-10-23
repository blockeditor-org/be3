use super::*;
use crate::reactive::*;

#[test]
fn a_scope_disposed_with_no_document_installed_still_removes_its_nodes() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal(None);
    let document = build_node(move || {
        let scope = Scope::new();
        let node = scope.run(|| {
            view! {
                <Frame width=10.0 height=10.0 />
            }
        });
        held.update(|held| *held = Some((scope, node)));
        view! {
            <Frame width=20.0 height=20.0 />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (scope, node) = held.update(|held| held.take().expect("the scope was built"));
    assert!(harness.document().contains(node));

    scope.dispose();
    harness.frame(Vec::new());
    assert!(
        !harness.document().contains(node),
        "the document removes the node once it next runs"
    );
}
