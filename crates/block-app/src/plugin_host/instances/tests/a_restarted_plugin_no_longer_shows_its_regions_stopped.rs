use super::*;

#[test]
fn a_restarted_plugin_no_longer_shows_its_regions_stopped() {
    let mut instances = placed();
    instances.next_screens(PASS);
    assert!(instances.shell_call(ShellCall::RegionStopped {
        instance: INSTANCE,
        region: REGION,
        stopped: block_plugin_api::Stopped::Closed,
    }));
    assert!(instances.stopped(INSTANCE, REGION).is_some());

    instances.reopen();

    assert!(
        instances.stopped(INSTANCE, REGION).is_none(),
        "a restart starts the region again"
    );
}
