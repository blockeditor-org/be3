//! An account's mailbox (guides/blocks/server.md, "Accounts"): what the account is told while
//! none of its devices may be connected, each message kept until a device of the account deletes
//! it. The server leaves each in the transaction of the change it tells: a notice of the ways to
//! sign in changing (`identity_changes.rs`), an accepted link's card (`accept-link`), a contact's
//! key change (`set-recovery-key`).
//!
//! A notice can't be deleted in its first `NOTICE_HOLD`: whoever it warns about may hold a
//! session too, so within it a device hides it only for itself. An account keeps at most
//! `MAX_ACCOUNT_NOTICES` notices, letting go of the oldest past their hold, so one in its hold
//! always stays.

use be_block_protocol::{ErrorCode, Letter, Message};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::{ServerStore, parse_uuid, portal::now};
use crate::ServerError;

/// Notices one account keeps past their hold; one in its hold always stays.
pub const MAX_ACCOUNT_NOTICES: i64 = 20;
/// How long a notice stays in the mailbox whatever a device deletes, in seconds.
pub const NOTICE_HOLD: i64 = 7 * 86_400;

fn kind(letter: &Letter) -> &'static str {
    match letter {
        Letter::Notice(_) => "notice",
        Letter::Accepted { .. } => "accepted",
        Letter::KeyChange(_) => "key-change",
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

/// Deletes what `a` and `b` left in each other's mailboxes, as they stop being contacts.
pub(crate) fn forget_between(connection: &Connection, a: Uuid, b: Uuid) -> Result<(), ServerError> {
    connection.execute(
        "DELETE FROM mailbox WHERE (account_id = ?1 AND sender_id = ?2)
            OR (account_id = ?2 AND sender_id = ?1)",
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
                    letter: be_block_protocol::decode(&letter)
                        .map_err(|_| ServerError::Corrupt)?,
                    created: created * 1000,
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
