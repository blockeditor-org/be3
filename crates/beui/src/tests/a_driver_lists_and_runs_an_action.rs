use super::*;
use crate::reactive::{Action, Chord, List, Text, build, create_signal, view};

#[test]
fn a_driver_lists_and_runs_an_action() {
    let document = build(|| {
        let (grouped, set_grouped) = create_signal(String::from("apart"));
        Action::new("test.group", "Group", move || set_grouped.set("grouped".to_owned()))
            .shortcut(Chord::ctrl(Key::G))
            .register();
        view! {
            <List spacing=0.0>
                <Text string={grouped} />
            </List>
        }
    });
    let mut driven = Driven::new(document);

    let listed = driven.ask(&["actions"]).expect("the actions are listed");
    assert!(
        listed.contains("test.group \"Group\" Ctrl+G"),
        "the action is listed with its label and shortcut:\n{listed}"
    );
    let changed = driven.ask(&["act", "test.group"]).expect("the action runs");
    assert!(changed.contains("+   Label value=\"grouped\""), "{changed}");
    assert!(driven.ask(&["act", "test.missing"]).is_err());
}
