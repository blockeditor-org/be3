use super::*;
use crate::selection::client;

#[test]
fn text_an_x11_program_copies_reaches_the_host_and_programs() {
    let mut harness = Harness::new();
    let _control = harness.client.data_control();
    copy_in_the_host(&mut harness, "copied in the host");
    harness.settle();

    copy_in_x11(&mut harness, SelectionTarget::Clipboard, &[client::TEXT]);
    harness.frame(Vec::new());
    assert_eq!(
        answer_x11(&mut harness, "copied in X11"),
        (SelectionTarget::Clipboard, client::TEXT.to_owned())
    );
    let mut copied = None;
    for _ in 0..4 {
        harness.frame(Vec::new());
        copied = copied.or_else(|| harness.output.as_mut()?.copied.take()?.plain_text());
    }
    assert_eq!(copied.as_deref(), Some("copied in X11"));
    harness.settle();
    assert!(
        harness
            .compositor()
            .server
            .state
            .xwayland
            .offered(SelectionTarget::Clipboard)
            .is_none(),
        "X11 keeps its own selection rather than being offered the host's copy of it"
    );

    let offer = client::selection().expect("programs are offered X11's selection");
    assert_eq!(client::mime_types(&offer), vec![client::TEXT.to_owned()]);
    let answer = client::ask(&offer);
    harness.settle();
    answer_x11(&mut harness, "copied in X11");
    assert_eq!(client::answer(answer), "copied in X11");

    harness
        .compositor()
        .server
        .state
        .x11_cleared(SelectionTarget::Clipboard);
    harness.settle();
    assert!(client::selection().is_none(), "X11 cleared its selection");
}
