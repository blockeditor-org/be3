use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_kept_subtree_moves_between_two_parents() {
    let _root = crate::reactive::RootScope::new();
    let (left, right) = (NodeRef::new(), NodeRef::new());
    let builds = create_signal(0);
    let on_left = create_signal(true);
    let document = build_node(move || {
        let panel = move || {
            view! {
                <Kept key="panel">|| {
                    builds.update(|builds| *builds += 1);
                    view! {
                        <Text>"panel"</Text>
                    }
                }</Kept>
            }
        };
        view! {
            <KeepAlive>
                <List spacing=0.0>
                    <List @node_ref=&left spacing=0.0>
                        <Show value={on_left}>|| { panel() }</Show>
                    </List>
                    <List @node_ref=&right spacing=0.0>
                        <Show value={move || !on_left.get()}>|| { panel() }</Show>
                    </List>
                </List>
            </KeepAlive>
        }
    });

    let (left, right) = (left.get(), right.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let kept = |harness: &Harness, list: NodeId| {
        harness
            .document()
            .children(list)
            .first()
            .and_then(|portal| harness.document().children(*portal).first().copied())
    };
    let built = kept(&harness, left).expect("the left list shows the kept panel");
    assert_eq!(kept(&harness, right), None);

    on_left.set(false);
    harness.frame(Vec::new());
    assert_eq!(kept(&harness, left), None);
    assert_eq!(
        kept(&harness, right),
        Some(built),
        "the panel moved to the other list is the node built first"
    );

    on_left.set(true);
    harness.frame(Vec::new());
    assert_eq!(kept(&harness, left), Some(built));
    assert_eq!(builds.get_untracked(), 1, "moving it never builds it again");
}
