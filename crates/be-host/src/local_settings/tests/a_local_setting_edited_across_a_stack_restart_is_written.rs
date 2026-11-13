use super::*;

#[test]
fn a_local_setting_edited_across_a_stack_restart_is_written() {
    let edit = DisplaySettings::default().set_mode(MONITOR, Some(SLOW));
    let mut sync = SettingsSync::<DisplaySettings>::default();
    sync.outboxes[0].queue(slow_mode());
    assert_eq!(sync.outboxes[0].send(4, &showing(FAST)), vec![edit.clone()]);

    // The stack restarts: the block opens again and its revisions count from one, so the
    // revision read next can be the one the edit was sent at without holding it.
    sync.heard(&be_host_stack::Changes::everything());
    assert_eq!(
        sync.outboxes[0].overlay(4, showing(FAST)),
        (showing(SLOW), vec![edit]),
        "the edit is written again"
    );
}
