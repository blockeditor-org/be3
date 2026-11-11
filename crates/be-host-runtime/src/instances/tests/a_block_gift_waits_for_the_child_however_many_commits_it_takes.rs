use super::*;

#[test]
fn a_block_gift_waits_for_the_child_however_many_commits_it_takes() {
    let mut instances = placed();
    instances.shell_call(ShellCall::FocusBlockChild {
        reply: reply(next_op()),
        instance: INSTANCE,
        block: Uuid::now_v7().into_bytes(),
    });
    assert!(
        instances.child_focus_pending(INSTANCE),
        "the editor asked for the keyboard for a child it has yet to place"
    );
    for _ in 0..8 {
        instances.settle_block_focus();
    }
    assert!(
        instances.child_focus_pending(INSTANCE),
        "a busy host can take any number of commits to place the child, and the keys typed \
         meanwhile wait for it"
    );
    instances.refuse_child_focus(INSTANCE);
    assert!(
        !instances.child_focus_pending(INSTANCE),
        "the person moving on ends it"
    );
}
