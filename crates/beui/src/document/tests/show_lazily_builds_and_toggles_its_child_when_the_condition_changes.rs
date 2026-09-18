use super::*;
use crate::reactive::{Button, Column, NodeRef, Show, Text, build, create_signal, intrinsic, view};

#[test]
fn show_lazily_builds_and_toggles_its_child_when_the_condition_changes() {
    let (column, toggle) = (NodeRef::new(), NodeRef::new());
    let builds = Rc::new(Cell::new(0));
    let sink = builds.clone();
    let document = build({
        let (column, toggle) = (column.clone(), toggle.clone());
        move || {
            let (visible, set_visible) = create_signal(false);
            view! {
                <Column @node_ref=&column spacing=0.0>
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
                            intrinsic(view! {
                                <Text string="panel" />
                            })
                        }}
                    />
                </Column>
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
    assert_eq!(builds.get(), 1, "hiding it again must not rebuild it");

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());
    assert_eq!(
        builds.get(),
        1,
        "showing it a second time must reuse the already-built child"
    );
    assert_eq!(
        harness.document().children(column),
        shown,
        "the child it shows again is the node it built the first time"
    );
}
