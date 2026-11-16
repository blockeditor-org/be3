//! Operator login links: `be-server login-link` makes one for an account, or for an email
//! with no account yet, in place of finding a mailed code in the server's log. A link works
//! once, for `LOGIN_LINK_LIFETIME`, and the server keeps only its hash. Using one signs the
//! browser in as recovering does: every other session ends, and the sign-in is a fresh proof.
//! A link for an email with no account makes the account, with no invite code, since only
//! the operator runs the command.
//!
//! `create` opens the data directory's database on its own, as `invite_codes` does, so the
//! command line can use it while the server runs.

use std::path::Path;
use std::time::Instant;

use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

use super::codes::{
    Identity, account_of, make_account, open_browser_session, recover_account, unregistered,
};
use super::{ServerStore, hash_token, invite_codes, normalize_email, parse_uuid, portal::now};
use crate::ServerError;

fn refused(message: &str) -> ServerError {
    ServerError::Refused(be_block_protocol::ErrorCode::InvalidRequest, message.into())
}

/// How long a link works, in seconds.
pub const LOGIN_LINK_LIFETIME: i64 = 15 * 60;

/// Whom a link signs in.
pub enum LinkFor<'a> {
    Account(Uuid),
    /// The account with this email, made when the link is used if there is none.
    Email(&'a str),
}

/// A link, before its origin.
pub struct LoginLink {
    /// Shown only now: the server keeps its hash.
    pub token: String,
    /// The email the account signs in with, or will be made with.
    pub email: String,
    /// Whether using it makes the account.
    pub registers: bool,
}

/// The path, below the portal's origin, that uses `token`.
pub fn path(token: &str) -> String {
    format!("/auth/link?token={token}")
}

/// Makes a link for `whom`.
pub fn create(data_dir: &Path, whom: LinkFor<'_>) -> Result<LoginLink, ServerError> {
    let database = invite_codes::open(data_dir)?;
    let (account, email) = match whom {
        LinkFor::Account(account) => {
            let email: Option<String> = database
                .query_row(
                    "SELECT COALESCE(email, '') FROM accounts WHERE id = ?1",
                    [account.to_string()],
                    |row| row.get(0),
                )
                .optional()?;
            let email =
                email.ok_or_else(|| refused(&format!("No account has the id {account}.")))?;
            (account.to_string(), email)
        }
        LinkFor::Email(email) => {
            let email = normalize_email(email)?;
            let account = account_of(&database, &email)?;
            if account.is_none() {
                // An agent's address, which nobody signs in with.
                unregistered(&database, &email)?;
            }
            (account.unwrap_or_default(), email)
        }
    };
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    database.execute("DELETE FROM login_links WHERE expires <= ?1", [now()])?;
    database.execute(
        "INSERT INTO login_links (token_hash, account_id, email, expires) VALUES (?1, ?2, ?3, ?4)",
        params![
            hash_token(&token),
            account,
            email,
            now() + LOGIN_LINK_LIFETIME
        ],
    )?;
    Ok(LoginLink {
        token,
        registers: account.is_empty(),
        email,
    })
}

/// A login link used: the browser session it opened, and what taking the account back removed.
pub struct UsedLink {
    pub session: String,
    pub account: Uuid,
    pub email: String,
    /// The ways to sign in it removed (`recover_account`), which the account is told of.
    pub removed: Vec<Identity>,
}

impl ServerStore {
    /// Uses a login link, once: makes the account first for a link made for a new email, then
    /// takes it back as a recovery does (`recover_account`) and answers a new browser session.
    pub async fn use_login_link(&self, token: &str, address: &str) -> Result<UsedLink, ServerError> {
        let token_hash = hash_token(token);
        let used = self
            .with_database(move |store, database| {
                let transaction = be_block_sqlite_schema::write_transaction(database)?;
                let row: Option<(String, String)> = transaction
                    .query_row(
                        "SELECT account_id, email FROM login_links WHERE token_hash = ?1 AND expires > ?2",
                        params![token_hash, now()],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?;
                let (account, email) = row.ok_or_else(|| {
                    refused("This link is invalid, expired, or already used. Ask the server's operator for a new one.")
                })?;
                transaction.execute("DELETE FROM login_links WHERE token_hash = ?1", [&token_hash])?;
                // A link for a new email signs in to whichever account has it by now.
                let account = match account.is_empty() {
                    false => account,
                    true => match account_of(&transaction, &email)? {
                        Some(account) => account,
                        None => {
                            unregistered(&transaction, &email)?;
                            let name = email.split('@').next().unwrap_or_default().to_owned();
                            make_account(&transaction, Some(&email), &name)?.to_string()
                        }
                    },
                };
                let account = parse_uuid(&account)?;
                let removed = recover_account(&transaction, account)?;
                let session = open_browser_session(&transaction, account)?;
                transaction.commit()?;
                store.sessions_changed();
                Ok(UsedLink { session, account, email, removed })
            })
            .await?;
        self.remembered.remember(&used.email, address, Instant::now());
        Ok(used)
    }
}
