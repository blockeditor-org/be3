use super::*;

#[test]
fn a_restarted_app_no_longer_shows_its_views_stopped() {
    let mut instances = placed();
    instances.next_screens(PASS);
    assert!(instances.shell_call(ShellCall::ViewStopped {
        reply: reply(next_op()),
        instance: INSTANCE,
        view: REGION,
        stopped: be_sdk_api::Stopped::Closed,
    }));
    assert!(instances.stopped(INSTANCE, REGION).is_some());

    instances.reopen();

    assert!(
        instances.stopped(INSTANCE, REGION).is_none(),
        "a restart starts the region again"
    );
}
