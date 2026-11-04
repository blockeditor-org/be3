use super::*;
use crate::selection::client;

#[test]
fn with_middle_click_paste_on_x11_and_wayland_programs_share_the_primary_selection() {
    let mut harness = Harness::new();
    harness.compositor().set_primary_selection(true);
    let control = harness.client.data_control();
    harness.settle();

    harness
        .client
        .select_with_control(&control, "selected in a program");
    harness.settle();
    let pasted = paste_in_x11(&mut harness, SelectionTarget::Primary, client::TEXT);
    assert_eq!(pasted, "selected in a program");

    copy_in_x11(&mut harness, SelectionTarget::Primary, &[client::TEXT]);
    harness.settle();
    let offer = client::control_primary().expect("programs are offered X11's selection");
    let answer = client::ask(&offer);
    harness.settle();
    assert_eq!(
        answer_x11(&mut harness, "selected in X11"),
        (SelectionTarget::Primary, client::TEXT.to_owned())
    );
    assert_eq!(client::answer(answer), "selected in X11");
}
