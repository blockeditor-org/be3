use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn a_disabled_action_ignores_its_shortcut_and_disables_its_button() {
    let _root = crate::reactive::RootScope::new();
    let runs = create_signal(0);
    let enabled = create_signal(false);
    let document = build_node(move || {
        let enabled = create_memo(move || enabled.get());
        let group = Action::new("test.group", "Group", move || {
            runs.update(|runs| *runs += 1)
        })
        .shortcut(Chord::ctrl(Key::G))
        .enabled(enabled)
        .register();
        view! {
            <List spacing=0.0>
                <Button @test_id={"group"} variant=ButtonVariant::Secondary action={group} />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };

    harness.key(Key::G, ctrl);
    harness.click(harness.center(harness.find("group")));
    assert_eq!(runs.get_untracked(), 0);
    assert!(text_within(harness.document(), harness.find("group"), "Group").is_some());

    enabled.set(true);
    harness.frame(Vec::new());
    harness.key(Key::G, ctrl);
    harness.click(harness.center(harness.find("group")));
    assert_eq!(runs.get_untracked(), 2);
}
