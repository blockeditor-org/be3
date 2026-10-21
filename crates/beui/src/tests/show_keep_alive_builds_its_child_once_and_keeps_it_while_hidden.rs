use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;

#[test]
fn show_keep_alive_builds_its_child_once_and_keeps_it_while_hidden() {
    let _root = crate::reactive::RootScope::new();
    let (column, toggle) = (NodeRef::new(), NodeRef::new());
    let builds = create_signal(0);
    let document = build_node(move || {
        let visible = create_signal(false);
        view! {
            <List @node_ref=&column spacing=0.0>
                <Button
                    @node_ref=&toggle
                    on_click={move || visible.update(|visible| *visible = !*visible)}
                >
                    <Text>"toggle"</Text>
                </Button>
                <ShowKeepAlive
                    condition={visible}
                    then={move || {
                        builds.update(|builds| *builds += 1);
                        view! {
                            <Text>"panel"</Text>
                        }
                    }}
                />
            </List>
        }
    });

    let (column, toggle) = (column.get(), toggle.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        builds.get_untracked(),
        0,
        "a hidden ShowKeepAlive must not build its child"
    );
    assert_eq!(
        harness.document().children(column).len(),
        1,
        "a hidden ShowKeepAlive puts no child among the children around it"
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
    assert_eq!(text_of(harness.document(), shown[1]), "panel");

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
    assert_eq!(
        harness.document().children(column),
        shown,
        "the child it shows again is the node it built the first time"
    );
}
