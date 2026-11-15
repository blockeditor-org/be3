//! What an account with no email is told on its devices in place of mail: a way to sign in
//! added or removed (plans/accounts.md, "Rules every method follows"). be-host shows each until
//! one of the account's devices dismisses it, which dismisses it everywhere.

use be_block_protocol::AccountNotice;
use rusqlite::params;
use uuid::Uuid;

use super::{ServerStore, portal::now};
use crate::ServerError;

/// Notices one account keeps; adding one past it lets go of the oldest.
pub const MAX_ACCOUNT_NOTICES: i64 = 20;

impl ServerStore {
    /// Adds a notice for `account`; the caller tells its connections
    /// (`Push::AccountNoticesChanged`).
    pub(crate) async fn add_account_notice(
        &self,
        account: Uuid,
        text: String,
    ) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            transaction.execute(
                "INSERT INTO account_notices (account_id, text, created) VALUES (?1, ?2, ?3)",
                params![account.to_string(), text, now()],
            )?;
            transaction.execute(
                "DELETE FROM account_notices WHERE account_id = ?1 AND id NOT IN (
                    SELECT id FROM account_notices WHERE account_id = ?1
                    ORDER BY id DESC LIMIT ?2)",
                params![account.to_string(), MAX_ACCOUNT_NOTICES],
            )?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    /// The account's notices not yet dismissed, oldest first.
    pub async fn account_notices(&self, account: Uuid) -> Result<Vec<AccountNotice>, ServerError> {
        self.with_database(move |_, database| {
            let mut query = database.prepare(
                "SELECT id, text, created FROM account_notices WHERE account_id = ?1 ORDER BY id",
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

    /// Dismisses one of the account's notices; another account's is left alone.
    pub async fn dismiss_account_notice(&self, account: Uuid, id: u64) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            database.execute(
                "DELETE FROM account_notices WHERE id = ?1 AND account_id = ?2",
                params![id as i64, account.to_string()],
            )?;
            Ok(())
        })
        .await
    }
}
