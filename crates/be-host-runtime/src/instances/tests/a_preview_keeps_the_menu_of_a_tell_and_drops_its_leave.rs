use super::*;
use be_sdk_api::shell_types::MenuSet;

#[test]
fn a_preview_keeps_the_menu_of_a_tell_and_drops_its_leave() {
    let menu = ShellMessage::Menu(MenuSet {
        view: ViewId::MAIN,
        items: Vec::new(),
    });
    let leave = ShellMessage::LeaveFrame;
    assert!(preview_may_tell(&menu), "a preview shows its menu");
    assert!(!preview_may_tell(&leave), "a preview acts on nothing");

    let mut instances = placed();
    assert!(instances.shell_call(ShellCall::Tell {
        reply: reply(next_op()),
        instance: INSTANCE,
        messages: vec![menu, leave],
    }));
    assert!(
        instances.take_leaving(INSTANCE),
        "a block that is not a preview takes every message of a tell"
    );
}
