use super::*;
use crate::reactive::{Action, Chord, NodeRef, action_scope, build, view};

#[component]
fn Scoped(name: String, hits: Rc<RefCell<Vec<String>>>) -> NodeId {
    let node = NodeRef::new();
    action_scope(&node);
    let ran = name.clone();
    Action::new(format!("{name}.go"), "Go", move || {
        hits.borrow_mut().push(ran.clone())
    })
    .shortcut(Chord::ctrl(Key::G))
    .register();
    view! {
        <List @node_ref=&node spacing=0.0>
            <unstyled::Button @test_id={format!("{name}.button")}>
                <ButtonFace label={name} />
            </unstyled::Button>
        </List>
    }
}

#[test]
fn an_action_shortcut_runs_in_the_scope_that_holds_the_focus() {
    let hits = Rc::new(RefCell::new(Vec::new()));
    let (first, second) = (hits.clone(), hits.clone());
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scoped name="first" hits={first} />
                <Scoped name="second" hits={second} />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(harness.find("second.button")));
    harness.key(
        Key::G,
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
    );
    harness.click(harness.center(harness.find("first.button")));
    harness.key(
        Key::G,
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
    );
    harness.key(Key::G, Modifiers::NONE);

    assert_eq!(
        *hits.borrow(),
        vec!["second".to_owned(), "first".to_owned()]
    );
}
