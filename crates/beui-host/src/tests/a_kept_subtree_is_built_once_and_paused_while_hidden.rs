use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_kept_subtree_is_built_once_and_paused_while_hidden() {
    let _root = crate::reactive::RootScope::new();
    let column = NodeRef::new();
    let builds = create_signal(0);
    let visible = create_signal(false);
    let ticks = create_signal(0);
    let seen = create_signal(0);
    let document = build_node(move || {
        view! {
            <KeepAlive>
                <List @node_ref=&column spacing=0.0>
                    <Text>"above"</Text>
                    <Show value={visible}>||
                        <Kept>|| {
                            builds.update(|builds| *builds += 1);
                            create_effect(move || {
                                ticks.get();
                                seen.update(|seen| *seen += 1);
                            });
                            view! {
                                <Text>"panel"</Text>
                            }
                        }</Kept>
                    </Show>
                </List>
            </KeepAlive>
        }
    });

    let column = column.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(
        builds.get_untracked(),
        0,
        "a kept subtree is built when first shown"
    );

    visible.set(true);
    harness.frame(Vec::new());
    assert_eq!(builds.get_untracked(), 1);
    let shown = harness.document().children(column);
    assert_eq!(shown.len(), 2);
    let panel = harness.document().children(shown[1]);
    assert_eq!(text_of(harness.document(), panel[0]), "panel");
    let ran = seen.get_untracked();

    visible.set(false);
    harness.frame(Vec::new());
    assert_eq!(harness.document().children(column).len(), 1);
    ticks.set(1);
    harness.frame(Vec::new());
    assert_eq!(
        seen.get_untracked(),
        ran,
        "nothing in a hidden kept subtree runs"
    );

    visible.set(true);
    harness.frame(Vec::new());
    assert_eq!(
        builds.get_untracked(),
        1,
        "showing it again reuses what it built"
    );
    let again = harness.document().children(column);
    assert_eq!(
        harness.document().children(again[1]),
        panel,
        "the node shown again is the one built the first time"
    );
    assert_eq!(
        seen.get_untracked(),
        ran + 1,
        "what changed while it was paused runs as it resumes"
    );
}
