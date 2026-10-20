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
    harness.client.flush();
    let app = &mut harness.app;
    let document = &mut harness.document;
    let output = harness.context.run(RawInput::default(), |context| {
        let rect = Rect::from_min_size(Pos2::ZERO, SCREEN);
        app.before(context, rect);
        document.show(context, rect);
        context.copy_text(text.to_owned());
        app.after(context, document);
    });
    harness.output = Some(output);
    harness.client.receive();
}
