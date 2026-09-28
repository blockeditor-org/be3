use super::*;
use crate::reactive::{List, NodeRef, build, view};

#[test]
fn an_empty_view_builds_a_children_prop_with_nothing_in_it() {
    let column = NodeRef::new();
    let document = build({
        let column = column.clone();
        move || {
            view! {
                <List
                    @node_ref=&column
                    spacing=0.0
                    children={view! {}}
                />
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert!(
        harness.document().children(column.get()).is_empty(),
        "an empty view! must build an empty Children"
    );
}
