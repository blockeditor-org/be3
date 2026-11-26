use super::*;

#[test]
fn a_failure_an_editor_reports_is_shown_as_a_notice() {
    let mut instances = placed();
    instances.next_screens(PASS);
    let message = format!("The picture could not be read ({:?}).", Uuid::new_v4());
    tell(&mut instances, ShellMessage::ReportFailure(message.clone()));
    let shown: Vec<_> = be_host_base::notices::shown()
        .into_iter()
        .filter(|notice| notice.message == message)
        .collect();
    assert_eq!(
        shown.len(),
        1,
        "the failure is shown once, as the host's own"
    );
    assert!(!shown[0].refusal, "it reads as a failure, not as a refusal");
}
