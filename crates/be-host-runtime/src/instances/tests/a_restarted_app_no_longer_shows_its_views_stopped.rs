use super::*;

#[test]
fn a_restarted_app_no_longer_shows_its_views_stopped() {
    let mut instances = placed();
    instances.next_screens(PASS);
    assert!(tell(
        &mut instances,
        ShellMessage::ViewStopped(be_sdk_api::shell_types::ViewEnd {
            view: REGION,
            stopped: be_sdk_api::Stopped::Closed,
        }),
    ));
    assert!(instances.stopped(INSTANCE, REGION).is_some());

    instances.reopen();

    assert!(
        instances.stopped(INSTANCE, REGION).is_none(),
        "a restart starts the region again"
    );
}
