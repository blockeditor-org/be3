use super::*;
use crate::selection::client;

#[test]
fn text_copied_outside_replaces_a_programs_selection_once_the_host_is_focused() {
    let mut harness = Harness::new();
    let control = harness.client.data_control();
    harness.settle();
    harness
        .client
        .copy_with_control(&control, "copied in a program");
    harness.settle();

    harness.frame(vec![Input::Focus(false)]);
    harness.frame(vec![Input::Focus(true)]);
    harness.settle();
    assert!(
        !client::source_cancelled(),
        "focus coming back without a copy outside leaves the program its selection"
    );

    harness.frame(vec![Input::Focus(false)]);
    harness
        .stepped
        .ask(&["clipboard", "copied outside"])
        .expect("the platform's clipboard is set");
    harness.frame(vec![Input::Focus(true)]);
    harness.settle();
    assert!(client::source_cancelled(), "the program's selection is replaced");
    let offer = client::selection().expect("the host's clipboard is offered");
    let answer = client::ask(&offer);
    harness.settle();
    assert_eq!(client::answer(answer), "copied outside");
}
