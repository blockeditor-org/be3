use super::*;
use crate::node::NodeId;
use crate::reactive::{
    ChildScope, ChildValue, Children, Prop, Scope, Text, build, component, on_cleanup, view,
};

struct Note {
    label: String,
    scope: ChildScope,
}

impl ChildValue for Note {
    fn adopt_scope(&mut self, scope: Scope) {
        self.scope.adopt(scope);
    }
}

crate::value_child_type!(Note);

#[component]
fn Noted(label: Prop<String>, disposals: WriteSignal<usize>) -> Note {
    on_cleanup(move || disposals.update(|disposals| *disposals += 1));
    Note {
        label: label.get(),
        scope: ChildScope::default(),
    }
}

#[component]
fn Legend(children: Children<Note>) -> NodeId {
    let labels = children
        .into_run()
        .peek()
        .iter()
        .map(|note| note.label.clone())
        .collect::<Vec<_>>()
        .join(", ");
    view! {
        <Text string={labels} />
    }
}

#[test]
fn a_component_that_builds_no_node_owns_its_scope_through_the_value() {
    let _root = crate::reactive::RootScope::new();
    let legend = NodeRef::new();
    let (disposals, set_disposals) = create_signal::<usize>(0);
    let document = build({
        move || {
            let loose = view! {
                <Noted label="loose" disposals={set_disposals} />
            };
            assert_eq!(loose.label, "loose");
            assert!(
                loose.scope.is_alive(),
                "a component that builds no node keeps its scope in the value it returns"
            );
            assert_eq!(disposals.get_untracked(), 0);
            drop(loose);
            assert_eq!(
                disposals.get_untracked(),
                1,
                "dropping the value must dispose the scope its component ran in"
            );

            view! {
                <Legend @node_ref=&legend>
                    <Noted label="first" disposals={set_disposals} />
                    <Noted label="second" disposals={set_disposals} />
                </Legend>
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        text_of(harness.document(), legend.get()),
        "first, second",
        "a slot typed for a node-less child receives the values its children built"
    );
    assert_eq!(
        disposals.get_untracked(),
        3,
        "a node-less child is disposed once the slot that read it drops the value"
    );
}
