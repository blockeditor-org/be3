use super::*;
use crate::reactive::{List, NodeRef, Text, build, view};

#[test]
fn a_lone_child_fills_a_children_prop_as_a_run_of_one() {
    let column = NodeRef::new();
    let only = NodeRef::new();
    let document = build({
        let column = column.clone();
        let only = only.clone();
        move || {
            let lone = view! {
                <Text @node_ref=&only string="One" font_size=14.0 color=Color32::WHITE />
            };
            view! {
                <List @node_ref=&column spacing=0.0 children={lone} />
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        harness.document().children(column.get()),
        vec![only.get()],
        "a single-root view! must fill a children prop as a run of one child"
    );
}
