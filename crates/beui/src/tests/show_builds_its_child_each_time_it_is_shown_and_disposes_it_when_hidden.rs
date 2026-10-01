use super::*;
use crate::reactive::{List, NodeRef, Show, Text, build, create_signal, view};
use crate::unstyled::Button;

#[test]
fn show_builds_its_child_each_time_it_is_shown_and_disposes_it_when_hidden() {
    let (column, toggle) = (NodeRef::new(), NodeRef::new());
    let builds = Rc::new(Cell::new(0));
    let sink = builds.clone();
    let document = build({
        let (column, toggle) = (column.clone(), toggle.clone());
        move || {
            let (visible, set_visible) = create_signal(false);
            view! {
                <List @node_ref=&column spacing=0.0>
                    <Button
                        @node_ref=&toggle
                        on_click={move || set_visible.update(|visible| *visible = !*visible)}
                    >
                        <Text string="toggle" />
                    </Button>
                    <Show
                        condition={visible}
                        then={move || {
                            sink.set(sink.get() + 1);
                            view! {
                                <Text string="panel" />
                            }
                        }}
                    />
                </List>
            }
        }
    });

    let (column, toggle) = (column.get(), toggle.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(builds.get(), 0, "a hidden show() must not build its child");
    assert_eq!(
        harness.document().children(column).len(),
        1,
        "a hidden show() puts no child among the children around it"
    );

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    assert_eq!(
        builds.get(),
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
        builds.get(),
        2,
        "showing it a second time must build it afresh"
    );
    let again = harness.document().children(column);
    assert_eq!(again.len(), 2);
    assert_ne!(again[1], shown[1], "the child it shows again is a new node");
    assert_eq!(text_of(harness.document(), again[1]), "panel");
}
