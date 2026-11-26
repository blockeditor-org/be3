//! An account's mailbox (guides/blocks/server.md, "Accounts"): what the account is told while
//! none of its devices may be connected, each message kept until a device of the account deletes
//! it. The server leaves each in the transaction of the change it tells: a notice of the ways to
//! sign in changing (`identity_changes.rs`), an accepted link's card (`accept-link`), a contact's
//! key change (`set-recovery-key`), and the account's records of contacts made (`accept-link`,
//! `create-agent`) or removed (`remove-contact`), which a device keeps in the account's block.
//!
//! A notice can't be deleted in its first `NOTICE_HOLD`: whoever it warns about may hold a
//! session too, so within it a device hides it only for itself. An account keeps at most
//! `MAX_ACCOUNT_NOTICES` notices, letting go of the oldest past their hold, so one in its hold
//! always stays. Letters of each kind are bounded per sender (`MAX_LETTERS_FROM_ONE`), so
//! with `MAX_CONTACTS_PER_ACCOUNT` per account. Past it, accepting another link is refused, but
//! a key change never is: a new phrase is what an account takes after a compromise, so a contact
//! that never reads its mailbox must not hold it up. The sender's oldest key change goes
//! instead, and the reader's device, finding the chain it follows broken, takes the contact as
//! not confirmed until they connect again by a link (`be_block_client::contacts::contacts`).
//! Times are kept in milliseconds.

use be_block_protocol::{ErrorCode, Letter, Message};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::{ServerStore, parse_uuid};
use crate::ServerError;

/// Notices one account keeps past their hold; one in its hold always stays.
pub const MAX_ACCOUNT_NOTICES: i64 = 20;
/// How long a notice stays in the mailbox whatever a device deletes, in milliseconds.
pub const NOTICE_HOLD: i64 = 7 * 86_400_000;
/// Letters of one kind one account holds from one sender: key changes, accepted cards or contact
/// records, not yet taken.
pub const MAX_LETTERS_FROM_ONE: i64 = 16;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as i64)
}

fn kind(letter: &Letter) -> &'static str {
    match letter {
        Letter::Notice(_) => "notice",
        Letter::Accepted { .. } => "accepted",
        Letter::KeyChange(_) => "key-change",
        Letter::Contact(_) => "contact",
        Letter::Removed => "removed",
        Letter::Pinned { .. } => "pinned",
    }
}

/// Leaves `letter` in `to`'s mailbox, from `from` (none for the server's notice), in the
/// caller's transaction. The caller tells `to`'s connections (`Push::MailboxChanged`).
pub(crate) fn leave(
    connection: &Connection,
    to: Uuid,
    from: Option<Uuid>,
    letter: &Letter,
) -> Result<(), ServerError> {
    let encoded = be_block_protocol::encode(letter).map_err(|_| ServerError::Corrupt)?;
    if let Some(from) = from {
        let kind = kind(letter);
        let held: i64 = connection
            .prepare_cached(
                "SELECT count(*) FROM mailbox WHERE account_id = ?1 AND sender_id = ?2 AND kind = ?3",
            )?
            .query_row(params![to.to_string(), from.to_string(), kind], |row| {
                row.get(0)
            })?;
        match letter {
            Letter::KeyChange(_) if held >= MAX_LETTERS_FROM_ONE => {
                connection.execute(
                    "DELETE FROM mailbox WHERE id IN (
                        SELECT id FROM mailbox
                        WHERE account_id = ?1 AND sender_id = ?2 AND kind = ?3
                        ORDER BY id LIMIT ?4)",
                    params![
                        to.to_string(),
                        from.to_string(),
                        kind,
                        held - MAX_LETTERS_FROM_ONE + 1
                    ],
                )?;
            }
            _ if held >= MAX_LETTERS_FROM_ONE => {
                return Err(crate::limits::too_many(
                    "letters of one kind one account holds from another not yet taken",
                    MAX_LETTERS_FROM_ONE,
                ));
            }
            _ => {}
        }
    }
    connection.execute(
        "INSERT INTO mailbox (account_id, sender_id, kind, letter, created)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            to.to_string(),
            from.map(|from| from.to_string()).unwrap_or_default(),
            kind(letter),
            encoded,
            now()
        ],
    )?;
    if let Letter::Notice(_) = letter {
        // Past the bound, only notices past their hold go.
        connection.execute(
            "DELETE FROM mailbox WHERE account_id = ?1 AND kind = 'notice' AND created <= ?3
                AND id NOT IN (
                    SELECT id FROM mailbox WHERE account_id = ?1 AND kind = 'notice'
                    ORDER BY id DESC LIMIT ?2)",
            params![to.to_string(), MAX_ACCOUNT_NOTICES, now() - NOTICE_HOLD],
        )?;
    }
    Ok(())
}

/// Deletes what `a` and `b` left in each other's mailboxes, as they stop being contacts. An
/// accepted link's card goes with the link itself, which `remove_contact` deletes in the same
/// transaction, so no link is left to finish without its card. A record of the other as a
/// contact stays, ahead of the `letter.removed` that follows it, so a device that takes both in
/// one pass keeps the removal with the newest record it ends, and no replay of that record
/// brings the contact back.
pub(crate) fn forget_between(connection: &Connection, a: Uuid, b: Uuid) -> Result<(), ServerError> {
    connection.execute(
        "DELETE FROM mailbox WHERE ((account_id = ?1 AND sender_id = ?2)
            OR (account_id = ?2 AND sender_id = ?1)) AND kind != 'contact'",
        params![a.to_string(), b.to_string()],
    )?;
    Ok(())
}

impl ServerStore {
    /// The account's mailbox, oldest first.
    pub async fn mailbox(&self, account: Uuid) -> Result<Vec<Message>, ServerError> {
        self.with_database(move |_, database| {
            let mut query = database.prepare(
                "SELECT id, sender_id, letter, created FROM mailbox
                 WHERE account_id = ?1 ORDER BY id",
            )?;
            let rows = query.query_map([account.to_string()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?;
            let mut messages = Vec::new();
            for row in rows {
                let (id, from, letter, created) = row?;
                messages.push(Message {
                    id: id as u64,
                    from: match from.is_empty() {
                        true => None,
                        false => Some(parse_uuid(&from)?),
                    },
                    letter: be_block_protocol::decode(&letter).map_err(|_| ServerError::Corrupt)?,
                    created,
                });
            }
            Ok(messages)
        })
        .await
    }

    /// Deletes a message from the account's mailbox; answers whether there was one to delete. A
    /// notice in its hold is refused.
    pub async fn delete_message(&self, account: Uuid, id: u64) -> Result<bool, ServerError> {
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let found: Option<(String, i64)> = transaction
                .query_row(
                    "SELECT kind, created FROM mailbox WHERE id = ?1 AND account_id = ?2",
                    params![id as i64, account.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let Some((kind, created)) = found else {
                return Ok(false);
            };
            if kind == "notice" && created > now() - NOTICE_HOLD {
                return Err(ServerError::Refused(
                    ErrorCode::InvalidRequest,
                    "a notice stays on every device for its first week".into(),
                ));
            }
            transaction.execute("DELETE FROM mailbox WHERE id = ?1", [id as i64])?;
            transaction.commit()?;
            Ok(true)
        })
        .await
    }
}
