use super::*;
use crate::selection::client;

#[test]
fn text_copied_outside_the_host_is_offered_to_programs() {
    let mut harness = Harness::new();
    let _control = harness.client.data_control();
    harness.settle();

    harness
        .stepped
        .ask(&["clipboard", "copied outside"])
        .expect("the platform's clipboard is set");
    harness.settle();

    let offer = client::selection().expect("the host's clipboard is offered");
    assert!(client::mime_types(&offer).contains(&client::TEXT.to_owned()));
    let answer = client::ask(&offer);
    harness.settle();
    assert_eq!(client::answer(answer), "copied outside");
}
