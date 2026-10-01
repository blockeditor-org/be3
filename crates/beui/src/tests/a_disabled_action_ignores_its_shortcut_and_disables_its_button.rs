use super::*;
use crate::reactive::{
    Action, Chord, build, create_memo, create_signal, view, with_reactive_scope,
};
use crate::styled::{Button, ButtonVariant};

#[test]
fn a_disabled_action_ignores_its_shortcut_and_disables_its_button() {
    let runs = Rc::new(Cell::new(0));
    let counted = runs.clone();
    let (enabled, set_enabled) = create_signal(false);
    let document = build(move || {
        let enabled = create_memo(move || enabled.get());
        let group = Action::new("test.group", "Group", move || {
            counted.set(counted.get() + 1)
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
    assert_eq!(runs.get(), 0);
    assert!(text_within(harness.document(), harness.find("group"), "Group").is_some());

    with_reactive_scope(harness.document_mut(), move || set_enabled.set(true));
    harness.frame(Vec::new());
    harness.key(Key::G, ctrl);
    harness.click(harness.center(harness.find("group")));
    assert_eq!(runs.get(), 2);
}
