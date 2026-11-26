use be_block_content::LogImport;

use super::*;

/// History an import brought from two chats (one imported, then exported again with its
/// history) folds each chat apart by its own places: the same place in each names its own
/// message, and the later chat's messages go after the earlier's, each reply naming its own.
#[test]
fn history_from_two_chats_folds_each_chat_apart() {
    const FIRST: Uuid = Uuid::from_u128(0xf125);
    const SECOND: Uuid = Uuid::from_u128(0x5ec0);
    let imported = |seq: u64, block: Uuid, original: u64, entry: &ChatEntry| {
        let mut item = signed(seq, ADA, entry, Signer::Verified);
        item.imported = Some(LogImport {
            block,
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
    let reply = ChatEntry::Message {
        time: 0,
        body: "answer there".to_owned(),
        reply_to: Some(0),
        attachments: Vec::new(),
    };
    let entries = [
        imported(0, FIRST, 0, &message("first chat")),
        imported(1, FIRST, 1, &edit(0, "first chat, edited")),
        imported(2, SECOND, 0, &message("second chat")),
        imported(3, SECOND, 1, &reply),
    ];
    let log = ChatLog::of(&entries);
    let history = log.history().expect("the chat has imported history");
    assert_eq!(
        bodies(&history.log),
        [
            Some("first chat, edited".to_owned()),
            Some("second chat".to_owned()),
            Some("answer there".to_owned()),
        ],
        "each chat's edit changes only its own message"
    );
    let answer = history
        .log
        .messages()
        .last()
        .expect("the second chat's reply");
    let answered = answer.reply_to.and_then(|seq| history.log.message(seq));
    assert_eq!(
        answered.and_then(|shown| shown.body.clone()),
        Some("second chat".to_owned()),
        "a reply names the message of its own chat"
    );
}
