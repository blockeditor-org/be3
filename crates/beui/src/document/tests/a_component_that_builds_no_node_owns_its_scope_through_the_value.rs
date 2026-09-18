use super::*;
use crate::node::NodeId;
use crate::reactive::{
    ChildScope, ChildValue, Children, Prop, Scope, Text, build, component, on_cleanup, view,
};

type Disposals = Rc<Cell<usize>>;

struct Note {
    label: String,
    scope: ChildScope,
}

impl ChildValue for Note {
    fn anchor(&self) -> Option<NodeId> {
        None
    }

    fn adopt_scope(&mut self, scope: Scope) {
        self.scope.adopt(scope);
    }
}

crate::child_type!(Note);

#[component]
fn Noted(label: Prop<String>, disposals: Disposals) -> Note {
    on_cleanup(move || disposals.set(disposals.get() + 1));
    Note {
        label: label.get(),
        scope: ChildScope::default(),
    }
}

#[component]
fn Legend(children: Children<Note>) -> NodeId {
    let labels = children
        .into_items()
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
    let legend = NodeRef::new();
    let disposals: Disposals = Rc::new(Cell::new(0));
    let document = build({
        let legend = legend.clone();
        let disposals = disposals.clone();
        move || {
            let loose = view! {
                <Noted label="loose" disposals={disposals.clone()} />
            };
            assert_eq!(loose.label, "loose");
            assert!(
                loose.scope.is_alive(),
                "a component that builds no node keeps its scope in the value it returns"
            );
            assert_eq!(disposals.get(), 0);
            drop(loose);
            assert_eq!(
                disposals.get(),
                1,
                "dropping the value must dispose the scope its component ran in"
            );

            view! {
                <Legend @node_ref=&legend>
                    <Noted label="first" disposals={disposals.clone()} />
                    <Noted label="second" disposals={disposals.clone()} />
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
        disposals.get(),
        3,
        "a node-less child is disposed once the slot that read it drops the value"
    );
}
