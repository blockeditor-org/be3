use super::*;
use crate::reactive::*;

#[component]
fn Scoped(name: String, hits: Signal<Vec<String>>) -> NodeId {
    let node = NodeRef::new();
    action_scope(&node);
    let ran = name.clone();
    Action::new(format!("{name}.go"), "Go", move || {
        hits.update(|hits| hits.push(ran.clone()))
    })
    .shortcut(Chord::ctrl(Key::G))
    .register();
    let button_id = format!("{name}.button");
    view! {
        <List @node_ref=&node spacing=0.0>
            <unstyled::Button @test_id=button_id>
                <ButtonFace label={name} />
            </unstyled::Button>
        </List>
    }
}

#[test]
fn an_action_shortcut_runs_in_the_scope_that_holds_the_focus() {
    let _root = crate::reactive::RootScope::new();
    let hits = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scoped name="first" hits={hits} />
                <Scoped name="second" hits={hits} />
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
        hits.get_untracked(),
        vec!["second".to_owned(), "first".to_owned()]
    );
}
