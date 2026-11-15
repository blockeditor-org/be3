//! What an account is told on its devices when its ways to sign in change (plans/accounts.md,
//! "Rules every method follows"), beside the mail an account with an email also gets, and the
//! history of those changes the portal's settings page lists.
//!
//! A notice can't be dismissed for everyone in its first `NOTICE_HOLD`: whoever it warns about
//! may hold a session too, so within it a dismissal hides it only on the device that dismissed
//! it. An account keeps at most `MAX_ACCOUNT_NOTICES`, letting go only of those dismissed or
//! past the hold, and changes its ways to sign in at most `IDENTITY_CHANGES_PER_DAY` times a
//! day, so nobody can push a fresh notice out by changing more.

use be_block_protocol::{AccountNotice, ErrorCode};
use rusqlite::params;
use uuid::Uuid;

use super::{ServerStore, portal::now};
use crate::ServerError;

/// Notices one account keeps of those dismissed or past their hold; one in its hold always stays.
pub const MAX_ACCOUNT_NOTICES: i64 = 20;
/// How long a notice stays on every device whatever is dismissed, in seconds.
pub const NOTICE_HOLD: i64 = 7 * 86_400;
/// Ways to sign in one account may add or remove in a day.
pub const IDENTITY_CHANGES_PER_DAY: i64 = 20;
/// How long the settings page lists a change to the ways to sign in, in seconds.
pub const IDENTITY_HISTORY: i64 = 30 * 86_400;

/// A way to sign in added or removed, as the settings page lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityChange {
    pub added: bool,
    pub label: String,
    /// Seconds since the epoch.
    pub created: i64,
}

impl ServerStore {
    /// Refuses another change to the account's ways to sign in once it made
    /// `IDENTITY_CHANGES_PER_DAY` in the last day.
    pub(crate) async fn identity_change_room(&self, account: Uuid) -> Result<(), ServerError> {
        let made: i64 = self
            .with_database(move |_, database| {
                Ok(database.query_row(
                    "SELECT count(*) FROM identity_changes WHERE account_id = ?1 AND created > ?2",
                    params![account.to_string(), now() - 86_400],
                    |row| row.get(0),
                )?)
            })
            .await?;
        match made >= IDENTITY_CHANGES_PER_DAY {
            true => Err(ServerError::Refused(
                ErrorCode::InvalidRequest,
                "Your ways to sign in changed too often today. Try again tomorrow.".into(),
            )),
            false => Ok(()),
        }
    }

    /// Records that a way to sign in was added or removed, and a notice of it for the account's
    /// devices; the caller tells its connections (`Push::AccountNoticesChanged`).
    pub(crate) async fn identity_changed(
        &self,
        account: Uuid,
        added: bool,
        label: String,
        text: String,
    ) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let (account, now) = (account.to_string(), now());
            transaction.execute(
                "INSERT INTO identity_changes (account_id, added, label, created) VALUES (?1, ?2, ?3, ?4)",
                params![account, added, label, now],
            )?;
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
                "SELECT added, label, created FROM identity_changes
                 WHERE account_id = ?1 AND created > ?2 ORDER BY id DESC",
            )?;
            let found = query
                .query_map(params![account.to_string(), now() - IDENTITY_HISTORY], |row| {
                    Ok(IdentityChange {
                        added: row.get(0)?,
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
