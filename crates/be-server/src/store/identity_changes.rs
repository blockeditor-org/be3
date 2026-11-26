//! The history of an account's ways to sign in changing, which the portal's settings page lists,
//! and the notice each change leaves in the account's mailbox (`store/mailbox.rs`) beside the
//! mail an account with an email also gets (guides/blocks/server.md, "Accounts").
//!
//! An account adds a way to sign in at most `IDENTITY_CHANGES_PER_DAY` times a day, counted in
//! the transaction that adds it (`claim_identity_add`), so nobody can push a fresh notice out by
//! changing more. Removals are not capped: the owner must always be able to remove what an
//! intruder added.

use be_block_protocol::{ErrorCode, Letter};
use rusqlite::{Connection, params};
use uuid::Uuid;

use super::{ServerStore, portal::now};
use crate::ServerError;

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

    /// Records changes of one kind to the account's ways to sign in, one per label, unless
    /// `claim_identity_add` already did (`recorded`), and one notice of them in the account's
    /// mailbox; the caller tells its connections (`Push::MailboxChanged`).
    pub(crate) async fn identity_changed(
        &self,
        account: Uuid,
        kind: ChangeKind,
        labels: Vec<String>,
        text: String,
        recorded: bool,
    ) -> Result<(), ServerError> {
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let account_id = account;
            let (account, now) = (account.to_string(), now());
            for label in labels.iter().filter(|_| !recorded) {
                transaction.execute(
                    "INSERT INTO identity_changes (account_id, kind, label, created) VALUES (?1, ?2, ?3, ?4)",
                    params![account, kind.stored(), label, now],
                )?;
            }
            transaction.execute(
                "DELETE FROM identity_changes WHERE account_id = ?1 AND created <= ?2",
                params![account, now - IDENTITY_HISTORY],
            )?;
            super::mailbox::leave(&transaction, account_id, None, &Letter::Notice(text))?;
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
                .query_map(
                    params![account.to_string(), now() - IDENTITY_HISTORY],
                    |row| {
                        Ok(IdentityChange {
                            kind: ChangeKind::read(&row.get::<_, String>(0)?),
                            label: row.get(1)?,
                            created: row.get(2)?,
                        })
                    },
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(found)
        })
        .await
    }
}
