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

use super::codes::{Identity, note_proof, open_browser_session, require_fresh_proof};
use super::{ServerStore, TokenKind, hash_token, parse_uuid, portal::now};
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

/// Passkeys one account may hold.
pub const MAX_PASSKEYS: i64 = 16;
/// The longest stored passkey: an ES256 or RS256 key with no attestation is well under it, and
/// a client that attaches a long attestation chain gets no room for one.
pub const MAX_CREDENTIAL_BYTES: usize = 4096;

/// The one answer to any passkey sign-in that fails, so it never tells a stranger which
/// credentials exist.
pub(crate) const DID_NOT_WORK: &str =
    "That passkey did not work here. Try again, or sign in another way.";

fn read(credential: &str) -> Result<Passkey, ServerError> {
    serde_json::from_str(credential).map_err(|_| ServerError::Corrupt)
}

/// A passkey found by its credential id.
pub(crate) struct Found {
    pub subject: String,
    pub account: Uuid,
    /// As it was when found, which the relying party checks the assertion with;
    /// `passkey_signed_in` checks its counter again against the stored one.
    pub passkey: Passkey,
}

/// Keeps what an assertion of the account's passkey `subject` changed (its counter, its
/// backup state), against the passkey as stored now, in the caller's transaction: an assertion
/// whose counter is not past the stored one is refused, so two at once can't both count, and a
/// cloned authenticator shows. Refused too when the passkey is no longer the account's.
fn note_use(
    database: &rusqlite::Connection,
    subject: &str,
    account: Uuid,
    result: &AuthenticationResult,
    refusal: &str,
) -> Result<(), ServerError> {
    let credential: Option<String> = database
        .query_row(
            "SELECT credential FROM identities WHERE kind = 'passkey' AND subject = ?1 AND account_id = ?2",
            params![subject, account.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    let credential = credential.ok_or_else(|| refused(refusal))?;
    let counter = serde_json::from_str::<serde_json::Value>(&credential)
        .ok()
        .and_then(|stored| stored["cred"]["counter"].as_u64())
        .ok_or(ServerError::Corrupt)?;
    if (result.counter() > 0 || counter > 0) && u64::from(result.counter()) <= counter {
        return Err(refused(refusal));
    }
    let mut passkey = read(&credential)?;
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
            let (account, credential) = row.ok_or_else(|| refused(DID_NOT_WORK))?;
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
        let browser = hash_token(TokenKind::Session, browser);
        let rp_id = rp_id.to_owned();
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            require_fresh_proof(&transaction, &browser, account)?;
            let held: i64 = transaction.query_row(
                "SELECT count(*) FROM identities WHERE account_id = ?1 AND kind = 'passkey'",
                [account.to_string()],
                |row| row.get(0),
            )?;
            if held >= MAX_PASSKEYS {
                return Err(refused(&format!(
                    "Your account has {MAX_PASSKEYS} passkeys, the most it may have. Remove one first."
                )));
            }
            let credential = stored(&passkey)?;
            if credential.len() > MAX_CREDENTIAL_BYTES {
                return Err(refused("That passkey did not work here. Try another."));
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
            super::identity_changes::claim_identity_add(
                &transaction,
                account,
                &crate::passkeys::label(identity.created, &identity.subject),
            )?;
            transaction.execute(
                "INSERT INTO identities (id, account_id, kind, subject, created, credential) VALUES (?1, ?2, 'passkey', ?3, ?4, ?5)",
                params![identity.id, account.to_string(), identity.subject, identity.created, credential],
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
                note_use(&transaction, &subject, found.account, &result, DID_NOT_WORK)?;
                let email: Option<String> = transaction.query_row(
                    "SELECT email FROM accounts WHERE id = ?1",
                    [found.account.to_string()],
                    |row| row.get(0),
                )?;
                let session = open_browser_session(&transaction, found.account)?;
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
        let browser = hash_token(TokenKind::Session, browser);
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            note_use(
                &transaction,
                &subject,
                account,
                &result,
                "That passkey did not confirm it is you. Try again, or another way.",
            )?;
            note_proof(&transaction, &browser, account)?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }
}
