use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;

#[test]
fn show_builds_its_child_each_time_it_is_shown_and_disposes_it_when_hidden() {
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
                <Show
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
        "a hidden show() must not build its child"
    );
    assert_eq!(
        harness.document().children(column).len(),
        1,
        "a hidden show() puts no child among the children around it"
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
    assert!(
        !harness.document().contains(shown[1]),
        "hiding it disposes of the child it built"
    );

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    assert_eq!(
        builds.get_untracked(),
        2,
        "showing it a second time must build it afresh"
    );
    let again = harness.document().children(column);
    assert_eq!(again.len(), 2);
    assert_ne!(again[1], shown[1], "the child it shows again is a new node");
    assert_eq!(text_of(harness.document(), again[1]), "panel");
}
