use be_block_content::LogImport;

use super::*;

/// History an import brought folds apart from the chat's own entries, by its places in the
/// block it was written in: an imported edit names only an imported message, a live edit only
/// a live one, and an imported entry that may not be history here (`Bad`) is hidden.
#[test]
fn imported_history_folds_apart_from_the_chats_own_messages() {
    const ELSEWHERE: Uuid = Uuid::from_u128(0xe15e);
    let imported = |seq: u64, original: u64, entry: &ChatEntry, signer: Signer| {
        let mut item = signed(seq, ADA, entry, signer);
        item.imported = Some(LogImport {
            block: ELSEWHERE,
            seq: original,
            importer: BO,
        });
        item
    };
    let edit = |message: u64, body: &str| ChatEntry::Edit {
        message,
        time: 0,
        body: body.to_owned(),
    };
    let entries = [
        imported(0, 4, &message("from the old chat"), Signer::Verified),
        imported(1, 5, &edit(4, "edited there"), Signer::Verified),
        item(2, ADA, &message("said here")),
        // An edit in the old chat naming its place 2, replayed: it changes nothing here.
        imported(3, 6, &edit(2, "replayed"), Signer::Verified),
        // One replayed after the chat's own entries is no one's, and hidden.
        imported(4, 7, &message("smuggled in"), Signer::Bad),
        // The chat's own edit naming place 4 names no imported message.
        item(5, ADA, &edit(4, "edited here")),
    ];
    let log = ChatLog::of(&entries);
    assert_eq!(bodies(&log), [Some("said here".to_owned())]);
    assert!(!log.message(2).unwrap().edited);
    let history = log.history().expect("the chat has imported history");
    assert_eq!(history.importer, BO);
    assert_eq!(
        bodies(&history.log),
        [Some("edited there".to_owned())],
        "only the imported edit changes the imported message"
    );
    assert_eq!(history.log.message(4).unwrap().seq, 4);
}
