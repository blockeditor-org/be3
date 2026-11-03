use super::*;
use crate::selection::client;

#[test]
fn with_middle_click_paste_off_x11_keeps_its_primary_selection_to_itself() {
    let mut harness = Harness::new();
    let _control = harness.client.data_control();
    harness.settle();

    copy_in_x11(&mut harness, SelectionTarget::Primary, &[client::TEXT]);
    harness.settle();

    assert!(
        client::control_primary().is_none(),
        "programs are offered nothing"
    );
    assert!(
        harness
            .compositor()
            .server
            .state
            .xwayland
            .offered(SelectionTarget::Primary)
            .is_none()
    );
}
