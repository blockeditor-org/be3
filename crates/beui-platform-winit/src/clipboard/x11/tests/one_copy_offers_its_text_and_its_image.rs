use super::*;

#[test]
fn one_copy_offers_its_text_and_its_image() {
    let Some(server) = Xvfb::start() else {
        return;
    };
    let _watchdog = Watchdog::start();
    let owner = Owner::connect(Some(&server.display)).expect("the owner connects");
    let reader = Reader::connect(&server.display);
    // Larger than one piece, so it goes by INCR.
    let png = (0..CHUNK * 2 + 7)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    owner.offer(Some("plain"), Some(&png));

    let targets = reader.targets();
    for target in ["UTF8_STRING", "text/plain;charset=utf-8", "image/png"] {
        assert!(
            targets.iter().any(|held| held == target),
            "{target} in {targets:?}"
        );
    }
    assert_eq!(reader.read("UTF8_STRING").as_deref(), Some(&b"plain"[..]));
    assert_eq!(
        reader.read_typed("TEXT").map(|(kind, _)| kind),
        Some(reader.atom("UTF8_STRING")),
        "TEXT is answered as UTF-8"
    );
    assert_eq!(
        reader.read("image/png"),
        Some(png),
        "the image arrives whole, beside the text"
    );

    owner.offer(Some("only text"), None);
    assert_eq!(
        reader.read("UTF8_STRING").as_deref(),
        Some(&b"only text"[..])
    );
    assert_eq!(reader.read("image/png"), None, "the old image is gone");
}
