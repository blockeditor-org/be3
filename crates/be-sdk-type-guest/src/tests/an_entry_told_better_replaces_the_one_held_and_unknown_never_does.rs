use super::*;

/// An entry heard again at its place replaces the one held when it is told better (who signed
/// it, once that can be told; its payload redacted), and never by a verdict that could not be
/// told: an offline read does not undo what was verified.
#[test]
fn an_entry_told_better_replaces_the_one_held_and_unknown_never_does() {
    use crate::editor::{HeldLog, Taken};
    use be_block_content::{LogItem, LogSigner};
    let item = |signer: LogSigner, payload: Option<&[u8]>| LogItem {
        seq: 3,
        id: Uuid::from_u128(7),
        author: Uuid::from_u128(1),
        arrival: 1_000,
        payload: payload.map(<[u8]>::to_vec),
        signer,
        imported: None,
    };
    let mut log = HeldLog::default();
    let mut take = |taken: LogItem| log.take(taken);
    assert_eq!(take(item(LogSigner::Unknown, Some(b"hi"))), Taken::New);
    assert_eq!(take(item(LogSigner::Unknown, Some(b"hi"))), Taken::Held);
    assert_eq!(take(item(LogSigner::Verified, Some(b"hi"))), Taken::Changed);
    assert_eq!(take(item(LogSigner::Unknown, None)), Taken::Held);
    assert_eq!(take(item(LogSigner::Redacted, None)), Taken::Changed);
    assert_eq!(log.items[&3], item(LogSigner::Redacted, None));
}
