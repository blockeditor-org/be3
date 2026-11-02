use super::*;

#[test]
fn a_crash_report_does_not_leave_a_dev_run_busy() {
    let state = AppStateStore::open(":memory:").expect("the app state opens");
    let mut app = BlockApp::with_state(state, String::new()).expect("the app starts");
    app.dev_workspace = true;
    assert_eq!(
        app.busy(),
        vec!["the dev workspace has yet to open".to_owned()],
        "a dev run is busy until its organization opens"
    );

    app.crashed("Block closed after this error the last time it ran.".to_owned());

    assert!(
        app.busy().is_empty(),
        "the error screen waits for a person, so a driver can read it rather than wait forever"
    );
}
