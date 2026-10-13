use super::*;
use crate::KeyAt;
use crate::reactive::{Action, Chord, NodeRef, build, view};

#[test]
fn an_action_placed_at_a_node_answers_only_while_the_focus_is_inside_it() {
    let _root = crate::reactive::RootScope::new();
    let runs = Rc::new(Cell::new(0));
    let (inside, outside, area) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build({
        let (runs, inside, outside, area) = (runs.clone(), inside, outside, area);
        move || {
            Action::new("test.placed", "Placed", move || runs.set(runs.get() + 1))
                .shortcut(Chord::ctrl(Key::J))
                .at(KeyAt::Node(area))
                .register();
            view! {
                <List spacing=8.0>
                    <List @node_ref=&area spacing=0.0>
                        <unstyled::Button @node_ref=&inside>
                            <ButtonFace label="Inside" />
                        </unstyled::Button>
                    </List>
                    <unstyled::Button @node_ref=&outside>
                        <ButtonFace label="Outside" />
                    </unstyled::Button>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(outside.get()));
    harness.key(Key::J, Modifiers::CTRL);
    assert_eq!(
        runs.get(),
        0,
        "the focus is outside the node the action is placed at"
    );

    harness.click(harness.center(inside.get()));
    harness.key(Key::J, Modifiers::CTRL);
    assert_eq!(runs.get(), 1, "with the focus inside, the action answers");
}
