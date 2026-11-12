use super::*;

#[test]
fn leaving_offline_says_it_needs_a_connection() {
    let mut shown = Shown::dialogs(HostUi {
        leaving: true,
        leave_refused: be_host_stack::OFFLINE_LEAVING.to_owned(),
        ..Default::default()
    });
    assert!(
        shown
            .stepped
            .document()
            .find_test_id("host.leave.refused")
            .is_some(),
        "the dialog where leaving was asked says why it did not happen"
    );
    shown.painted("leaving_offline_says_it_needs_a_connection");
}
