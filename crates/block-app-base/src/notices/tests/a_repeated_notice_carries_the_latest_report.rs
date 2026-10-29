use super::*;

#[test]
fn a_repeated_notice_carries_the_latest_report() {
    let message = "Repeated stopped: the first line is the same".to_owned();
    report_with(message.clone(), "the first crash".to_owned());
    report_with(message.clone(), "the second crash".to_owned());

    let toasts: Vec<Notice> = shown()
        .into_iter()
        .filter(|toast| toast.message == message)
        .collect();
    assert_eq!(toasts.len(), 1, "the same message shows one toast");
    assert_eq!(
        report_of(toasts[0].id).as_deref(),
        Some("the second crash"),
        "Copy report copies the crash that happened last"
    );
}
