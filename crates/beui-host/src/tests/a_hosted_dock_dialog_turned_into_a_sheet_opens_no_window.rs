use super::*;

#[test]
fn a_hosted_dock_dialog_turned_into_a_sheet_opens_no_window() {
    let _root = crate::reactive::RootScope::new();
    let Dock {
        mut harness,
        window,
        ..
    } = hosted_dock(WIDE_VIEWPORT);
    harness.resize(vec2(390.0, 844.0));
    for _ in 0..4 {
        harness.frame(Vec::new());
    }
    let requests = harness.take_window_requests();
    assert!(
        requests.contains(&WindowRequest::Close(window)),
        "on a phone its window closes: {requests:?}"
    );
    assert!(
        !requests
            .iter()
            .any(|request| matches!(request, WindowRequest::Open { .. })),
        "and no other window opens for its sheet: {requests:?}"
    );
}
