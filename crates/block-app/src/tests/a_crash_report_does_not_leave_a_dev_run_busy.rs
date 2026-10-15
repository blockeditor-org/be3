use super::*;

#[test]
fn a_crash_report_does_not_leave_a_dev_run_busy() {
    let state = AppStateStore::open(":memory:").expect("the app state opens");
    let mut app = BlockApp::with_state(state, String::new()).expect("the app starts");
    app.dev_workspace = true;
    assert!(app.busy(), "a dev run is busy until its workspace opens");

    app.crashed("Block closed after this error the last time it ran.".to_owned());

    assert!(
        !app.busy(),
        "the error screen waits for a person, so a driver can read it rather than wait forever"
    );
}
