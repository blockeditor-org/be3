use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_driver_lists_and_runs_an_action() {
    let document = build_node(|| {
        let grouped = create_signal(String::from("apart"));
        Action::new("test.group", "Group", move || {
            grouped.set("grouped".to_owned())
        })
        .shortcut(Chord::ctrl(Key::G))
        .register();
        view! {
            <List spacing=0.0>
                <Text>{grouped}</Text>
            </List>
        }
    });
    let mut driven = stepped(crate::native::testing::Showing(document));

    let listed = driven.ask(&["actions"]).expect("the actions are listed");
    assert!(
        listed.contains("test.group \"Group\" Ctrl+G"),
        "the action is listed with its label and shortcut:\n{listed}"
    );
    let changed = driven.ask(&["act", "test.group"]).expect("the action runs");
    assert!(changed.contains("+   Label value=\"grouped\""), "{changed}");
    assert!(driven.ask(&["act", "test.missing"]).is_err());
}
