use super::*;
use be_sdk_api::shell_types::{BlockAct, MenuSet};

#[test]
fn a_preview_keeps_the_menu_of_a_tell_and_drops_its_act() {
    let menu = ShellMessage::Menu(MenuSet {
        view: ViewId::MAIN,
        items: Vec::new(),
    });
    let act = ShellMessage::Act(BlockAct {
        block: [1; 16],
        command: ShellCommand::Share,
    });
    assert!(preview_may_tell(&menu), "a preview shows its menu");
    assert!(!preview_may_tell(&act), "a preview acts on nothing");

    let mut instances = placed();
    assert!(instances.shell_call(ShellCall::Tell {
        reply: reply(next_op()),
        instance: INSTANCE,
        messages: vec![menu, act],
    }));
    assert_eq!(
        instances.take_shell_commands(INSTANCE),
        vec![(Uuid::from_bytes([1; 16]), ShellCommand::Share)],
        "a block that is not a preview takes every message of a tell"
    );
}
