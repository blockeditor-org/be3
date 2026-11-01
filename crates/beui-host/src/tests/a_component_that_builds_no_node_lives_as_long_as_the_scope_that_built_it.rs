use super::*;
use crate::primitives::*;
use crate::reactive::*;
use beui::NodeId;

struct Note {
    label: String,
}

crate::value_child_type!(Note);

#[component]
fn Noted(label: Prop<String>, disposals: Signal<usize>) -> Note {
    on_cleanup(move || disposals.update(|disposals| *disposals += 1));
    Note {
        label: label.get_untracked(),
    }
}

#[component]
fn Legend(children: Children<Note>) -> NodeId {
    let notes = children.into_run();
    let labels = create_memo(move || {
        notes
            .items()
            .iter()
            .map(|note| note.label.clone())
            .collect::<Vec<_>>()
            .join(", ")
    });
    view! {
        <Text>{labels}</Text>
    }
}

#[test]
fn a_component_that_builds_no_node_lives_as_long_as_the_scope_that_built_it() {
    let _root = crate::reactive::RootScope::new();
    let legend = NodeRef::new();
    let disposals = create_signal::<usize>(0);
    let shown = create_signal(true);
    let document = build_node(move || {
        let loose = view! {
            <Noted label="loose" disposals={disposals} />
        };
        assert_eq!(loose.label, "loose");
        drop(loose);
        assert_eq!(
            disposals.get_untracked(),
            0,
            "the value does not own its component's scope; the scope that built it does"
        );

        view! {
            <Legend @node_ref=&legend>
                <Noted label="first" disposals={disposals} />
                <Show condition={shown}>
                    <Noted label="second" disposals={disposals} />
                </Show>
            </Legend>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(
        text_of(harness.document(), legend.get()),
        "first, second",
        "a slot typed for a node-less child receives the values its children built"
    );

    shown.set(false);
    harness.frame(Vec::new());
    assert_eq!(text_of(harness.document(), legend.get()), "first");
    assert_eq!(
        disposals.get_untracked(),
        1,
        "hiding the show disposes the branch it built, and the value's component with it"
    );
}
