use super::*;
use crate::selection::client;

#[test]
fn text_the_host_copies_is_offered_to_programs() {
    let mut harness = Harness::new();
    let _control = harness.client.data_control();
    harness.settle();

    copy_in_the_host(&mut harness, "copied in the host");
    harness.settle();

    let offer = client::selection().expect("the host's text is offered");
    assert!(client::mime_types(&offer).contains(&client::TEXT.to_owned()));
    let answer = client::ask(&offer);
    harness.settle();
    assert_eq!(client::answer(answer), "copied in the host");
}

pub(super) fn copy_in_the_host(harness: &mut Harness, text: &str) {
    harness.stepped.app_mut().copy = Some(text.to_owned());
    harness.frame(Vec::new());
}
