use super::*;
use crate::selection::client;
use beui::ClipItem;
use beui::clipboard::formats;

#[test]
fn every_format_the_host_copies_is_offered_to_programs() {
    let mut harness = Harness::new();
    let _control = harness.client.data_control();
    harness.settle();

    harness.stepped.program_mut().copy = Some(
        beui::Clip::new()
            .with(ClipItem::new(formats::TEXT_FRAGMENT, b"fragment".to_vec()))
            .with(ClipItem::text("plain"))
            .with(ClipItem::png(b"PNG bytes".to_vec())),
    );
    harness.frame(Vec::new());
    harness.settle();

    let offer = client::selection().expect("the host's clip is offered");
    let offered = client::mime_types(&offer);
    for mime in [client::TEXT, formats::PNG, formats::TEXT_FRAGMENT] {
        assert!(offered.contains(&mime.to_owned()), "{mime} in {offered:?}");
    }
    for (mime, data) in [
        (formats::PNG, "PNG bytes"),
        (formats::TEXT_FRAGMENT, "fragment"),
        (client::TEXT, "plain"),
    ] {
        let answer = client::ask_as(&offer, mime);
        harness.settle();
        assert_eq!(client::answer(answer), data, "as {mime}");
    }
}
