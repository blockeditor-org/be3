use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;

#[test]
fn a_kept_child_of_a_show_is_built_once_and_kept_while_hidden() {
    let _root = crate::reactive::RootScope::new();
    let (column, toggle) = (NodeRef::new(), NodeRef::new());
    let builds = create_signal(0);
    let document = build_node(move || {
        let visible = create_signal(false);
        view! {
            <KeepAlive>
                <List @node_ref=&column spacing=0.0>
                    <Button
                        @node_ref=&toggle
                        on_click={move || visible.update(|visible| *visible = !*visible)}
                    >
                        <Text>"toggle"</Text>
                    </Button>
                    <Show value={visible}>
                        <Kept>|| {
                            builds.update(|builds| *builds += 1);
                            view! {
                                <Text>"panel"</Text>
                            }
                        }</Kept>
                    </Show>
                </List>
            </KeepAlive>
        }
        .into_child()
    });

    let (column, toggle) = (column.get(), toggle.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        builds.get_untracked(),
        0,
        "a kept child under a hidden Show is not built"
    );
    assert_eq!(
        harness.document().children(column).len(),
        1,
        "a hidden Show puts no child among the children around it"
    );

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    assert_eq!(
        builds.get_untracked(),
        1,
        "showing it for the first time must build it"
    );
    let shown = harness.document().children(column);
    assert_eq!(shown.len(), 2);
    let panel = harness.document().children(shown[1]);
    assert_eq!(text_of(harness.document(), panel[0]), "panel");

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().children(column).len(),
        1,
        "hiding it again takes its child back out of the list"
    );
    assert_eq!(
        builds.get_untracked(),
        1,
        "hiding it again must not rebuild it"
    );

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    assert_eq!(
        builds.get_untracked(),
        1,
        "showing it a second time must reuse the already-built child"
    );
    let again = harness.document().children(column);
    assert_eq!(again.len(), 2);
    assert_eq!(
        harness.document().children(again[1]),
        panel,
        "the child it shows again is the node it built the first time"
    );
}
