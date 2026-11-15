//! Passkeys: the accounts half (`passkeys.rs` is the ceremony half). A passkey is an identity
//! of kind `passkey` whose subject is `"{rp_id} {credential id}"`, the id base64url without
//! padding, so one made for another RP ID (before `--auth-origin` changed) is never found, and whose
//! `credential` holds the key webauthn-rs checks assertions with and its signature counter. A
//! credential id belongs to one account at most. The account's id is the passkey's user
//! handle, so a discoverable sign-in names the account it is for; the credential id decides.

use base64::Engine;
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;
use webauthn_rs::prelude::{AuthenticationResult, Passkey};

use super::codes::{FRESH_PROOF, Identity, open_browser_session};
use super::{ServerStore, hash_token, parse_uuid, portal::now};
use crate::ServerError;

fn refused(message: &str) -> ServerError {
    ServerError::Refused(be_block_protocol::ErrorCode::InvalidRequest, message.into())
}

/// A passkey identity's subject: the RP ID it was made for and its credential id.
pub(crate) fn subject(rp_id: &str, credential_id: &[u8]) -> String {
    format!(
        "{rp_id} {}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(credential_id)
    )
}

fn stored(passkey: &Passkey) -> Result<String, ServerError> {
    serde_json::to_string(passkey).map_err(|_| ServerError::Corrupt)
}

fn read(credential: &str) -> Result<Passkey, ServerError> {
    serde_json::from_str(credential).map_err(|_| ServerError::Corrupt)
}

/// A passkey found by its credential id.
pub(crate) struct Found {
    pub subject: String,
    pub account: Uuid,
    pub passkey: Passkey,
}

/// Keeps what an assertion changed about the passkey `subject` (its counter, its backup
/// state), if anything.
fn note_use(
    database: &rusqlite::Connection,
    subject: &str,
    mut passkey: Passkey,
    result: &AuthenticationResult,
) -> Result<(), ServerError> {
    if passkey.update_credential(result) == Some(true) {
        database.execute(
            "UPDATE identities SET credential = ?2 WHERE kind = 'passkey' AND subject = ?1",
            params![subject, stored(&passkey)?],
        )?;
    }
    Ok(())
}

impl ServerStore {
    /// The account's passkeys for `rp_id`.
    pub(crate) async fn passkeys(
        &self,
        rp_id: &str,
        account: Uuid,
    ) -> Result<Vec<Passkey>, ServerError> {
        let prefix = format!("{rp_id} ");
        self.with_database(move |_, database| {
            let mut query = database.prepare(
                "SELECT credential FROM identities WHERE account_id = ?1 AND kind = 'passkey' AND substr(subject, 1, length(?2)) = ?2 ORDER BY created, id",
            )?;
            let found = query
                .query_map(params![account.to_string(), prefix], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            found.iter().map(|credential| read(credential)).collect()
        })
        .await
    }

    /// The passkey for `rp_id` with this credential id, and the account it signs in to.
    pub(crate) async fn passkey(
        &self,
        rp_id: &str,
        credential_id: &[u8],
    ) -> Result<Found, ServerError> {
        let subject = subject(rp_id, credential_id);
        self.with_database(move |_, database| {
            let row: Option<(String, String)> = database
                .query_row(
                    "SELECT account_id, credential FROM identities WHERE kind = 'passkey' AND subject = ?1",
                    [&subject],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let (account, credential) = row.ok_or_else(|| {
                refused("That passkey does not sign in to an account here. Sign in another way.")
            })?;
            Ok(Found {
                subject,
                account: parse_uuid(&account)?,
                passkey: read(&credential)?,
            })
        })
        .await
    }

    /// Adds a passkey the browser just made to the account, if the browser is still the
    /// account's and proved so just now. Answers the identity added, for the notice.
    pub(crate) async fn add_passkey(
        &self,
        rp_id: &str,
        account: Uuid,
        browser: &str,
        passkey: Passkey,
    ) -> Result<Identity, ServerError> {
        let browser = hash_token(browser);
        let rp_id = rp_id.to_owned();
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let fresh: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM sessions WHERE token_hash = ?1 AND account_id = ?2 AND kind = 'browser' AND expires > ?3 AND proved > ?4)",
                params![browser, account.to_string(), now(), now() - FRESH_PROOF],
                |row| row.get(0),
            )?;
            if !fresh {
                return Err(refused(
                    "Confirm it is you first, with one of your ways to sign in.",
                ));
            }
            let subject = subject(&rp_id, passkey.cred_id());
            let taken: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM identities WHERE kind = 'passkey' AND subject = ?1)",
                [&subject],
                |row| row.get(0),
            )?;
            if taken {
                return Err(refused("That passkey is already a way to sign in here."));
            }
            let identity = Identity {
                id: Uuid::new_v4().to_string(),
                kind: "passkey".into(),
                subject,
                email: String::new(),
                created: now(),
            };
            transaction.execute(
                "INSERT INTO identities (id, account_id, kind, subject, created, credential) VALUES (?1, ?2, 'passkey', ?3, ?4, ?5)",
                params![identity.id, account.to_string(), identity.subject, identity.created, stored(&passkey)?],
            )?;
            transaction.commit()?;
            Ok(identity)
        })
        .await
    }

    /// Signs in with a passkey whose assertion the relying party accepted: a new browser
    /// session for its account, proved just now, if the passkey is still the account's.
    /// Answers the session and the account's email, whose budget the address no longer spends.
    pub(crate) async fn passkey_signed_in(
        &self,
        found: Found,
        result: AuthenticationResult,
        address: &str,
    ) -> Result<String, ServerError> {
        let subject = found.subject.clone();
        let (session, email) = self
            .with_database(move |_, database| {
                let transaction = be_block_sqlite_schema::write_transaction(database)?;
                let held: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM identities WHERE kind = 'passkey' AND subject = ?1 AND account_id = ?2)",
                    params![subject, found.account.to_string()],
                    |row| row.get(0),
                )?;
                if !held {
                    return Err(refused(
                        "That passkey does not sign in to an account here. Sign in another way.",
                    ));
                }
                note_use(&transaction, &subject, found.passkey, &result)?;
                let email: Option<String> = transaction.query_row(
                    "SELECT email FROM accounts WHERE id = ?1",
                    [found.account.to_string()],
                    |row| row.get(0),
                )?;
                let session = open_browser_session(&transaction, found.account, false)?;
                transaction.commit()?;
                Ok((session, email.unwrap_or_default()))
            })
            .await?;
        if !email.is_empty() {
            self.remembered
                .remember(&email, address, std::time::Instant::now());
        }
        Ok(session)
    }

    /// Proves the browser is the account's owner with one of its passkeys, whose assertion the
    /// relying party accepted, if the passkey is still the account's.
    pub(crate) async fn passkey_proved(
        &self,
        rp_id: &str,
        account: Uuid,
        browser: &str,
        result: AuthenticationResult,
    ) -> Result<(), ServerError> {
        let subject = subject(rp_id, result.cred_id());
        let browser = hash_token(browser);
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let credential: Option<String> = transaction
                .query_row(
                    "SELECT credential FROM identities WHERE kind = 'passkey' AND subject = ?1 AND account_id = ?2",
                    params![subject, account.to_string()],
                    |row| row.get(0),
                )
                .optional()?;
            let credential =
                credential.ok_or_else(|| refused("That passkey does not sign in to your account."))?;
            note_use(&transaction, &subject, read(&credential)?, &result)?;
            let now = now();
            let proved = transaction.execute(
                "UPDATE sessions SET proved = ?3 WHERE token_hash = ?1 AND account_id = ?2 AND kind = 'browser' AND expires > ?3",
                params![browser, account.to_string(), now],
            )?;
            if proved == 0 {
                return Err(refused("Sign in to continue."));
            }
            transaction.commit()?;
            Ok(())
        })
        .await
    }
}
