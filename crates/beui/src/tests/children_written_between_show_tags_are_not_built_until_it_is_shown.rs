use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;

#[component]
fn CountedPanel(builds: Signal<usize>) -> NodeId {
    builds.update(|builds| *builds += 1);
    view! {
        <Text>"panel"</Text>
    }
}

#[test]
fn children_written_between_show_tags_are_not_built_until_it_is_shown() {
    let _root = crate::reactive::RootScope::new();
    let builds = create_signal::<usize>(0);
    let (toggle, panel) = (NodeRef::new(), NodeRef::new());

    let document = build_node(move || {
        let visible = create_signal(false);
        view! {
            <List @node_ref=&panel spacing=0.0>
                <Button
                    @node_ref=&toggle
                    on_click={move || visible.update(|visible| *visible = !*visible)}
                >
                    <Text>"toggle"</Text>
                </Button>
                <Show condition=visible>
                    <CountedPanel builds={builds} />
                </Show>
            </List>
        }
    });

    let (toggle, column) = (toggle.get(), panel.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        builds.get_untracked(),
        0,
        "a children block filling a render prop must stay unbuilt while it is hidden"
    );
    assert_eq!(harness.document().children(column).len(), 1);

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    assert_eq!(
        builds.get_untracked(),
        1,
        "showing it must build the children block"
    );
    let shown = harness.document().children(column);
    assert_eq!(shown.len(), 2);
    assert_eq!(text_of(harness.document(), shown[1]), "panel");

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    assert_eq!(
        builds.get_untracked(),
        2,
        "hiding and showing it again must build the children block afresh"
    );
}
