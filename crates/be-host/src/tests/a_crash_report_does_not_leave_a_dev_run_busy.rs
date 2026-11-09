use super::*;

#[test]
fn a_crash_report_does_not_leave_a_dev_run_busy() {
    let state = HostStateStore::open(":memory:").expect("the host state opens");
    let mut core = HostCore::with_state(state, String::new()).expect("the app starts");
    core.dev_workspace = true;
    // The block store's own startup work comes and goes on its schedule, and other tests
    // share it, so only the host's own reasons are compared.
    assert_eq!(
        core.own_busy(),
        vec!["the dev workspace has yet to open".to_owned()],
        "a dev run is busy until its organization opens"
    );

    core.crashed("be-host closed after this error the last time it ran.".to_owned());

    assert!(
        core.busy().is_empty(),
        "the error screen waits for a person, so a driver can read it rather than wait forever"
    );
}
