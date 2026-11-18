use super::*;

/// A message shows who signed it as the least trusted of the entries it shows: an edit signed
/// with a replaced key marks it, and one signed as nobody's changes nothing.
#[test]
fn a_message_shows_as_trusted_as_the_least_trusted_entry_it_shows() {
    let edit = |body: &str| ChatEntry::Edit {
        message: 0,
        time: 0,
        body: body.to_owned(),
    };
    let entries = [
        item(0, ADA, &message("hello")),
        signed(1, ADA, &edit("forged"), LogSigner::Bad),
    ];
    let log = ChatLog::of(&entries);
    assert_eq!(log.message(0).unwrap().body.as_deref(), Some("hello"));
    assert_eq!(log.message(0).unwrap().signer, LogSigner::Verified);

    let entries = [
        item(0, ADA, &message("hello")),
        signed(1, ADA, &edit("with an old key"), LogSigner::Replaced),
    ];
    let log = ChatLog::of(&entries);
    assert_eq!(
        log.message(0).unwrap().body.as_deref(),
        Some("with an old key")
    );
    assert_eq!(log.message(0).unwrap().signer, LogSigner::Replaced);
}
