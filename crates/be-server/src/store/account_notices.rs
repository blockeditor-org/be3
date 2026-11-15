//! What an account is told on its devices when its ways to sign in change (guides/blocks/server.md,
//! "Accounts"), beside the mail an account with an email also gets, and the
//! history of those changes the portal's settings page lists.
//!
//! A notice can't be dismissed for everyone in its first `NOTICE_HOLD`: whoever it warns about
//! may hold a session too, so within it a dismissal hides it only on the device that dismissed
//! it. An account keeps at most `MAX_ACCOUNT_NOTICES`, letting go only of those dismissed or
//! past the hold, and adds a way to sign in at most `IDENTITY_CHANGES_PER_DAY` times a day,
//! counted in the transaction that adds it (`claim_identity_add`), so nobody can push a fresh
//! notice out by changing more. Removals are not capped: the owner must always be able to
//! remove what an intruder added.

use be_block_protocol::{AccountNotice, ErrorCode};
use rusqlite::{Connection, params};
use uuid::Uuid;

use super::{ServerStore, portal::now};
use crate::ServerError;

/// Notices one account keeps of those dismissed or past their hold; one in its hold always stays.
pub const MAX_ACCOUNT_NOTICES: i64 = 20;
/// How long a notice stays on every device whatever is dismissed, in seconds.
pub const NOTICE_HOLD: i64 = 7 * 86_400;
/// Ways to sign in one account may add in a day.
pub const IDENTITY_CHANGES_PER_DAY: i64 = 20;
/// How long the settings page lists a change to the ways to sign in, in seconds.
pub const IDENTITY_HISTORY: i64 = 30 * 86_400;

/// What happened to an account's ways to sign in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Removed,
    /// Its email changed; the label is the new one.
    Email,
}

impl ChangeKind {
    fn stored(self) -> &'static str {
        match self {
            ChangeKind::Added => "added",
            ChangeKind::Removed => "removed",
            ChangeKind::Email => "email",
        }
    }

    fn read(stored: &str) -> Self {
        match stored {
            "added" => ChangeKind::Added,
            "email" => ChangeKind::Email,
            _ => ChangeKind::Removed,
        }
    }
}

/// A change to an account's ways to sign in, as the settings page lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityChange {
    pub kind: ChangeKind,
    pub label: String,
    /// Seconds since the epoch.
    pub created: i64,
}

fn too_many_added() -> ServerError {
    ServerError::Refused(
        ErrorCode::InvalidRequest,
        "Too many ways to sign in were added today. Try again tomorrow.".into(),
    )
}

fn added_today(connection: &Connection, account: Uuid) -> Result<i64, ServerError> {
    Ok(connection.query_row(
        "SELECT count(*) FROM identity_changes
         WHERE account_id = ?1 AND kind = 'added' AND created > ?2",
        params![account.to_string(), now() - 86_400],
        |row| row.get(0),
    )?)
}

/// Records adding a way to sign in, in the transaction that adds it, refusing it once the
/// account added `IDENTITY_CHANGES_PER_DAY` in the last day: counted where it is recorded, so
/// two at once can't both slip under the cap.
pub(crate) fn claim_identity_add(
    connection: &Connection,
    account: Uuid,
    label: &str,
) -> Result<(), ServerError> {
    if added_today(connection, account)? >= IDENTITY_CHANGES_PER_DAY {
        return Err(too_many_added());
    }
    connection.execute(
        "INSERT INTO identity_changes (account_id, kind, label, created) VALUES (?1, 'added', ?2, ?3)",
        params![account.to_string(), label, now()],
    )?;
    Ok(())
}

impl ServerStore {
    /// Refuses, before it begins, adding a way to sign in the account may not add today
    /// (`claim_identity_add` checks again where it is added).
    pub(crate) async fn identity_add_room(&self, account: Uuid) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            match added_today(database, account)? >= IDENTITY_CHANGES_PER_DAY {
                true => Err(too_many_added()),
                false => Ok(()),
            }
        })
        .await
    }

    /// Records a change to the account's ways to sign in, unless `claim_identity_add` already
    /// did (`recorded`), and a notice of it for the account's devices; the caller tells its
    /// connections (`Push::AccountNoticesChanged`).
    pub(crate) async fn identity_changed(
        &self,
        account: Uuid,
        kind: ChangeKind,
        label: String,
        text: String,
        recorded: bool,
    ) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let (account, now) = (account.to_string(), now());
            if !recorded {
                transaction.execute(
                    "INSERT INTO identity_changes (account_id, kind, label, created) VALUES (?1, ?2, ?3, ?4)",
                    params![account, kind.stored(), label, now],
                )?;
            }
            transaction.execute(
                "DELETE FROM identity_changes WHERE account_id = ?1 AND created <= ?2",
                params![account, now - IDENTITY_HISTORY],
            )?;
            transaction.execute(
                "INSERT INTO account_notices (account_id, text, created) VALUES (?1, ?2, ?3)",
                params![account, text, now],
            )?;
            // Past the bound, only what is dismissed or past its hold goes.
            transaction.execute(
                "DELETE FROM account_notices WHERE account_id = ?1
                    AND (dismissed = 1 OR created <= ?3)
                    AND id NOT IN (
                        SELECT id FROM account_notices WHERE account_id = ?1
                        ORDER BY id DESC LIMIT ?2)",
                params![account, MAX_ACCOUNT_NOTICES, now - NOTICE_HOLD],
            )?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    /// The account's ways to sign in added or removed in the last `IDENTITY_HISTORY`, newest
    /// first.
    pub(crate) async fn identity_changes(
        &self,
        account: Uuid,
    ) -> Result<Vec<IdentityChange>, ServerError> {
        self.with_database(move |_, database| {
            let mut query = database.prepare(
                "SELECT kind, label, created FROM identity_changes
                 WHERE account_id = ?1 AND created > ?2 ORDER BY id DESC",
            )?;
            let found = query
                .query_map(params![account.to_string(), now() - IDENTITY_HISTORY], |row| {
                    Ok(IdentityChange {
                        kind: ChangeKind::read(&row.get::<_, String>(0)?),
                        label: row.get(1)?,
                        created: row.get(2)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(found)
        })
        .await
    }

    /// The account's notices not dismissed for everyone, oldest first.
    pub async fn account_notices(&self, account: Uuid) -> Result<Vec<AccountNotice>, ServerError> {
        self.with_database(move |_, database| {
            let mut query = database.prepare(
                "SELECT id, text, created FROM account_notices
                 WHERE account_id = ?1 AND dismissed = 0 ORDER BY id",
            )?;
            let found = query
                .query_map([account.to_string()], |row| {
                    Ok(AccountNotice {
                        id: row.get::<_, i64>(0)? as u64,
                        text: row.get(1)?,
                        created: row.get::<_, i64>(2)? * 1000,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(found)
        })
        .await
    }

    /// Dismisses one of the account's notices for every device, once it is past its hold;
    /// answers whether it did. Within the hold, or for another account's, nothing changes.
    pub async fn dismiss_account_notice(&self, account: Uuid, id: u64) -> Result<bool, ServerError> {
        self.with_database(move |_, database| {
            let changed = database.execute(
                "UPDATE account_notices SET dismissed = 1
                 WHERE id = ?1 AND account_id = ?2 AND dismissed = 0 AND created <= ?3",
                params![id as i64, account.to_string(), now() - NOTICE_HOLD],
            )?;
            Ok(changed > 0)
        })
        .await
    }
}
