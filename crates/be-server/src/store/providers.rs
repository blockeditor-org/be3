//! Signing in with an OpenID Connect provider: the accounts half (`oidc.rs` is the network
//! half). A provider identity is keyed on the configured issuer and the token's `sub`, never on
//! email. A provider's email counts only when the operator marks it authoritative for email
//! and the token says it is verified; then it becomes the account's verified email under the
//! rule every email follows: no two accounts hold it. A sign-in whose verified email an account
//! already holds makes no account and links nothing on its own: it mails that email a code,
//! and entering it signs that account in and links the provider (`CodePurpose::Link`).
//! A new account waits for the operator's invite code (`oidc_pending`).

use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::codes::{
    CodePurpose, CodeRequest, MailedCode, account_of, add_email_identity, make_account,
    open_browser_session, unregistered,
};
use super::{ServerStore, hash_token, invite_codes, parse_uuid, portal::now};
use crate::oidc::{Provider, Verified};
use crate::{ServerConfig, ServerError};

/// How long a begun sign-in waits for the provider to send the browser back.
pub const PROVIDER_SIGN_IN_LIFETIME: i64 = 600;
/// How long a provider sign-in with no account waits for the invite code.
pub const PENDING_LIFETIME: i64 = 1800;
/// Begun sign-ins one address (an IPv6 /64) may have waiting.
pub const PROVIDER_SIGN_INS_PER_ADDRESS: i64 = 16;
/// Begun sign-ins and pending registrations the server keeps at once.
pub const MAX_PROVIDER_SIGN_INS: i64 = 4096;

/// Whether an identity still signs in: a provider's only while the server has it (one of
/// `issuers`), and a passkey only at the RP ID it was made for (`rp_id`, the portal's now).
pub(crate) fn signs_in(kind: &str, subject: &str, issuers: &[String], rp_id: Option<&str>) -> bool {
    match kind {
        "oidc" => issuers
            .iter()
            .any(|issuer| subject.starts_with(&format!("{issuer} "))),
        "passkey" => rp_id.is_some_and(|rp_id| subject.starts_with(&format!("{rp_id} "))),
        _ => true,
    }
}

fn refused(message: &str) -> ServerError {
    ServerError::Refused(be_block_protocol::ErrorCode::InvalidRequest, message.into())
}

/// What a sign-in with a provider is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    /// Signing in, or making an account.
    SignIn,
    /// Adding the provider to the signed-in account, after a fresh proof.
    Link,
    /// Proving it is the signed-in account's owner, as a code to its email does.
    Proof,
}

impl Intent {
    fn name(self) -> &'static str {
        match self {
            Intent::SignIn => "sign-in",
            Intent::Link => "link",
            Intent::Proof => "proof",
        }
    }

    fn of(name: &str) -> Result<Self, ServerError> {
        match name {
            "sign-in" => Ok(Intent::SignIn),
            "link" => Ok(Intent::Link),
            "proof" => Ok(Intent::Proof),
            _ => Err(ServerError::Corrupt),
        }
    }
}

/// A sign-in begun with a provider, taken back when the provider sends the browser back.
pub(crate) struct Begun {
    pub verifier: String,
    pub nonce: String,
    pub device_code: String,
    pub intent: Intent,
    /// For a link or a proof, the account that began it.
    pub account: Option<Uuid>,
}

/// What a checked provider sign-in did.
pub(crate) enum Outcome {
    /// A browser session for the account; `device_code` is the browser sign-in to return to.
    /// `moved_from` is the account's email before the provider's changed email replaced it,
    /// which is told.
    SignedIn {
        session: String,
        device_code: String,
        moved_from: Option<String>,
    },
    /// The provider was added to the signed-in account.
    Linked,
    /// The signed-in browser proved it is the account's owner.
    Proved,
    /// An account holds the provider's verified email: this code to it links the provider.
    LinkByCode(MailedCode),
    /// No account has the identity: the browser's attempt waits for an invite code.
    NeedsInvite,
}

/// What the settings page's links to begin a link or a proof carry: only the browser's own
/// session makes it, so a page elsewhere can't begin one for it.
pub(crate) fn start_check(browser: &str) -> String {
    hash_token(&format!("be3 provider start {browser}"))
}

/// Whether `given` is the browser's `start_check`. It compares their hashes, so how long the
/// comparison takes says nothing about the check itself.
pub(crate) fn is_start_check(browser: &str, given: &str) -> bool {
    hash_token(given) == hash_token(&start_check(browser))
}

/// A provider identity's subject: the configured issuer and the token's `sub`.
pub fn subject(issuer: &str, sub: &str) -> String {
    format!("{issuer} {sub}")
}

/// Adds a provider identity to an account, refusing one another account holds. `email` is
/// the verified email it gave, if the provider is authoritative for email: an account with no
/// email takes it when no other account holds it. The identity keeps the email only when its
/// account holds it.
pub(crate) fn add_provider_identity(
    database: &Connection,
    account: Uuid,
    subject: &str,
    email: &str,
) -> Result<(), ServerError> {
    let holder: Option<String> = database
        .query_row(
            "SELECT account_id FROM identities WHERE kind = 'oidc' AND subject = ?1",
            [subject],
            |row| row.get(0),
        )
        .optional()?;
    match holder {
        Some(holder) if holder == account.to_string() => return Ok(()),
        Some(_) => {
            return Err(refused(
                "That provider account already signs in to another account.",
            ));
        }
        None => {}
    }
    let account_email: Option<String> = database.query_row(
        "SELECT email FROM accounts WHERE id = ?1",
        [account.to_string()],
        |row| row.get(0),
    )?;
    let takes =
        !email.is_empty() && account_email.is_none() && unregistered(database, email).is_ok();
    let holds = !email.is_empty() && account_email.as_deref() == Some(email);
    database.execute(
        "INSERT INTO identities (id, account_id, kind, subject, created, email) VALUES (?1, ?2, 'oidc', ?3, ?4, ?5)",
        params![
            Uuid::new_v4().to_string(),
            account.to_string(),
            subject,
            now(),
            if takes || holds { email } else { "" }
        ],
    )?;
    if takes {
        database.execute(
            "UPDATE accounts SET email = ?2 WHERE id = ?1",
            params![account.to_string(), email],
        )?;
        add_email_identity(database, &account.to_string(), email)?;
    }
    Ok(())
}

/// Follows a provider's changed verified email. Where the account's email was the provider's
/// old one and no other account holds the new one, the account's email, and the email it signs
/// in with, move to the new one; an account with no email never gains one this way, since
/// its owner may have removed it. The identity keeps the email only while its account holds
/// it. Answers the address the account's email moved from, which is told.
fn follow_email(
    database: &Connection,
    account: &str,
    subject: &str,
    stored: &str,
    email: &str,
) -> Result<Option<String>, ServerError> {
    if email == stored {
        return Ok(None);
    }
    let account_email: Option<String> = database.query_row(
        "SELECT email FROM accounts WHERE id = ?1",
        [account],
        |row| row.get(0),
    )?;
    let mut moved_from = None;
    let kept = if account_email.as_deref() == Some(email) {
        email
    } else if !stored.is_empty()
        && account_email.as_deref() == Some(stored)
        && unregistered(database, email).is_ok()
    {
        database.execute(
            "UPDATE accounts SET email = ?2 WHERE id = ?1",
            params![account, email],
        )?;
        database.execute(
            "UPDATE identities SET subject = ?3 WHERE account_id = ?1 AND kind = 'email' AND subject = ?2",
            params![account, stored, email],
        )?;
        moved_from = Some(stored.to_owned());
        email
    } else if !stored.is_empty() && account_email.as_deref() == Some(stored) {
        // Another account holds the new one: the identity keeps the email its account holds,
        // so a later free one still moves it.
        stored
    } else {
        ""
    };
    database.execute(
        "UPDATE identities SET email = ?2 WHERE kind = 'oidc' AND subject = ?1",
        params![subject, kept],
    )?;
    Ok(moved_from)
}

/// Clears expired begun sign-ins and refuses a new one past the caps: per address (an IPv6
/// /64) and overall.
fn room_for_sign_in(database: &Connection, address: &str) -> Result<(), ServerError> {
    let now = now();
    database.execute("DELETE FROM oidc_states WHERE expires <= ?1", [now])?;
    database.execute("DELETE FROM oidc_pending WHERE expires <= ?1", [now])?;
    let from_address: i64 = database.query_row(
        "SELECT count(*) FROM oidc_states WHERE address = ?1",
        [address],
        |row| row.get(0),
    )?;
    let waiting: i64 = database.query_row(
        "SELECT (SELECT count(*) FROM oidc_states) + (SELECT count(*) FROM oidc_pending)",
        [],
        |row| row.get(0),
    )?;
    if from_address >= PROVIDER_SIGN_INS_PER_ADDRESS || waiting >= MAX_PROVIDER_SIGN_INS {
        return Err(refused(
            "Too many sign-ins are waiting on a provider. Try again in a while.",
        ));
    }
    Ok(())
}

impl ServerStore {
    /// Refuses a sign-in past the caps before it calls the provider, so strangers can't make
    /// the server call providers beyond them.
    pub(crate) async fn provider_room(&self, address: &str) -> Result<(), ServerError> {
        let address = crate::address::address_key_text(address);
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            room_for_sign_in(&transaction, &address)?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    /// Keeps a begun sign-in's secrets until its callback, bound to the browser's attempt.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn begin_provider(
        &self,
        provider: &str,
        state: &str,
        verifier: &str,
        nonce: &str,
        attempt: &str,
        address: &str,
        device_code: &str,
        intent: Intent,
        account: Option<Uuid>,
    ) -> Result<(), ServerError> {
        let (provider, state, verifier, nonce, device_code) = (
            provider.to_owned(),
            hash_token(state),
            verifier.to_owned(),
            nonce.to_owned(),
            device_code.to_owned(),
        );
        let attempt = hash_token(attempt);
        let address = crate::address::address_key_text(address);
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            room_for_sign_in(&transaction, &address)?;
            let now = now();
            transaction.execute(
                "INSERT INTO oidc_states (state_hash, provider, verifier, nonce, attempt_hash, device_code, intent, link_account, address, expires) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    state,
                    provider,
                    verifier,
                    nonce,
                    attempt,
                    device_code,
                    intent.name(),
                    account.map(|account| account.to_string()).unwrap_or_default(),
                    address,
                    now + PROVIDER_SIGN_IN_LIFETIME,
                ],
            )?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }

    /// Takes back a begun sign-in, once: only in the browser that began it, and only for the
    /// provider it began with.
    pub(crate) async fn take_provider_state(
        &self,
        provider: &str,
        state: &str,
        attempt: &str,
    ) -> Result<Begun, ServerError> {
        let (provider, state, attempt) =
            (provider.to_owned(), hash_token(state), hash_token(attempt));
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            #[allow(clippy::type_complexity)]
            let row: Option<(String, String, String, String, String, String, String)> = transaction
                .query_row(
                    "SELECT provider, verifier, nonce, attempt_hash, device_code, intent, link_account FROM oidc_states WHERE state_hash = ?1 AND expires > ?2",
                    params![state, now()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
                )
                .optional()?;
            transaction.execute("DELETE FROM oidc_states WHERE state_hash = ?1", [&state])?;
            transaction.commit()?;
            let Some((begun_with, verifier, nonce, began_in, device_code, intent, account)) = row
            else {
                return Err(refused("This sign-in expired or was already used. Start again."));
            };
            if begun_with != provider || began_in != attempt {
                return Err(refused("This sign-in began in another browser. Start again here."));
            }
            Ok(Begun {
                verifier,
                nonce,
                device_code,
                intent: Intent::of(&intent)?,
                account: match account.is_empty() {
                    true => None,
                    false => Some(parse_uuid(&account)?),
                },
            })
        })
        .await
    }

    /// Does what a checked provider sign-in asks. `browser` is the browser's session, which a
    /// link or a proof is for.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn provider_verified(
        &self,
        provider: &Provider,
        verified: Verified,
        begun: Begun,
        browser: &str,
        attempt: &str,
        address: &str,
        config: &ServerConfig,
    ) -> Result<Outcome, ServerError> {
        let subject = subject(&provider.issuer, &verified.subject);
        let email = verified.email.clone().unwrap_or_default();
        match begun.intent {
            Intent::Link => {
                let account = begun
                    .account
                    .ok_or_else(|| refused("Sign in to continue."))?;
                // The provider's name, as the account's notice and history name it.
                let name = provider.name.clone();
                let browser = hash_token(browser);
                self.with_database(move |store, database| {
                    let transaction = be_block_sqlite_schema::write_transaction(database)?;
                    // The browser must still be the account that began it, and have proved so
                    // just now: a session that changed or went stale meanwhile links nothing.
                    let fresh: bool = transaction.query_row(
                        "SELECT EXISTS(SELECT 1 FROM sessions WHERE token_hash = ?1 AND account_id = ?2 AND kind = 'browser' AND expires > ?3 AND proved > ?4)",
                        params![browser, account.to_string(), now(), now() - super::codes::FRESH_PROOF],
                        |row| row.get(0),
                    )?;
                    if !fresh {
                        return Err(refused(
                            "Confirm it is you first, with a code to your email, a passkey or a provider you sign in with.",
                        ));
                    }
                    add_provider_identity(&transaction, account, &subject, &email)?;
                    super::account_notices::claim_identity_add(&transaction, account, &name)?;
                    transaction.commit()?;
                    store.sessions_changed();
                    Ok(Outcome::Linked)
                })
                .await
            }
            Intent::Proof => {
                let account = begun
                    .account
                    .ok_or_else(|| refused("Sign in to continue."))?;
                let browser = hash_token(browser);
                self.with_database(move |_, database| {
                    let linked: bool = database.query_row(
                        "SELECT EXISTS(SELECT 1 FROM identities WHERE kind = 'oidc' AND subject = ?1 AND account_id = ?2)",
                        params![subject, account.to_string()],
                        |row| row.get(0),
                    )?;
                    if !linked {
                        return Err(refused("That provider account does not sign in to yours."));
                    }
                    let now = now();
                    let proved = database.execute(
                        "UPDATE sessions SET proved = ?3 WHERE token_hash = ?1 AND account_id = ?2 AND kind = 'browser' AND expires > ?3",
                        params![browser, account.to_string(), now],
                    )?;
                    match proved {
                        0 => Err(refused("Sign in to continue.")),
                        _ => Ok(Outcome::Proved),
                    }
                })
                .await
            }
            Intent::SignIn => {
                if !config.allow_login {
                    return Err(refused("This server is not accepting sign-ins."));
                }
                let allow_registration = config.allow_registration;
                let (provider_id, name) = (provider.id.clone(), verified.name.clone());
                let (pending_attempt, device_code) = (hash_token(attempt), begun.device_code);
                let found = self
                    .with_database(move |_, database| {
                        let transaction = be_block_sqlite_schema::write_transaction(database)?;
                        let held: Option<(String, String)> = transaction
                            .query_row(
                                "SELECT account_id, email FROM identities WHERE kind = 'oidc' AND subject = ?1",
                                [&subject],
                                |row| Ok((row.get(0)?, row.get(1)?)),
                            )
                            .optional()?;
                        if let Some((account, stored)) = held {
                            let moved_from = match email.is_empty() {
                                true => None,
                                false => follow_email(&transaction, &account, &subject, &stored, &email)?,
                            };
                            let session =
                                open_browser_session(&transaction, parse_uuid(&account)?, false)?;
                            transaction.commit()?;
                            return Ok(Ok(Outcome::SignedIn { session, device_code, moved_from }));
                        }
                        // Its verified email has an account: link only after a code to it.
                        if !email.is_empty() && unregistered(&transaction, &email).is_err() {
                            return match account_of(&transaction, &email)? {
                                Some(_) => Ok(Err((subject, email, device_code))),
                                None => Err(refused(
                                    "Email: that address belongs to an account that can't add a provider.",
                                )),
                            };
                        }
                        if !allow_registration {
                            return Err(refused("This server is not accepting new accounts."));
                        }
                        transaction.execute(
                            "INSERT OR REPLACE INTO oidc_pending (attempt_hash, provider, subject, email, display_name, device_code, expires) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                            params![pending_attempt, provider_id, subject, email, name, device_code, now() + PENDING_LIFETIME],
                        )?;
                        transaction.commit()?;
                        Ok(Ok(Outcome::NeedsInvite))
                    })
                    .await?;
                match found {
                    Ok(outcome) => Ok(outcome),
                    Err((subject, email, device_code)) => {
                        let mailed = self
                            .request_code_with(
                                CodeRequest {
                                    purpose: CodePurpose::Link,
                                    email: &email,
                                    account: None,
                                    display_name: "",
                                    invite: "",
                                    device_code: &device_code,
                                    attempt,
                                    address,
                                },
                                subject,
                                email.clone(),
                            )
                            .await?
                            .ok_or_else(|| refused("Start again, or sign in another way."))?;
                        Ok(Outcome::LinkByCode(mailed))
                    }
                }
            }
        }
    }

    /// The provider and suggested name of the browser's provider sign-in that waits for an
    /// invite code.
    pub async fn pending_provider(&self, attempt: &str) -> Option<(String, String)> {
        let attempt = hash_token(attempt);
        self.with_database(move |_, database| {
            Ok(database
                .query_row(
                    "SELECT provider, display_name FROM oidc_pending WHERE attempt_hash = ?1 AND expires > ?2",
                    params![attempt, now()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?)
        })
        .await
        .ok()
        .flatten()
    }

    /// Makes the account a provider sign-in waits on, with the operator's invite code: a
    /// browser session for it, and the browser sign-in to return to.
    pub async fn register_by_provider(
        &self,
        attempt: &str,
        invite: &str,
        display_name: &str,
    ) -> Result<(String, String), ServerError> {
        super::portal::validate_name(display_name)?;
        let (attempt, invite, name) = (
            hash_token(attempt),
            invite_codes::invite_hash(invite),
            display_name.trim().to_owned(),
        );
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let row: Option<(String, String, String)> = transaction
                .query_row(
                    "SELECT subject, email, device_code FROM oidc_pending WHERE attempt_hash = ?1 AND expires > ?2",
                    params![attempt, now()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            let (subject, email, device_code) = row.ok_or_else(|| {
                refused("This sign-in expired. Sign in with your provider again.")
            })?;
            invite_codes::check(&transaction, &invite)?;
            invite_codes::spend(&transaction, &invite)?;
            let account = make_account(&transaction, None, &name)?;
            add_provider_identity(&transaction, account, &subject, &email)?;
            transaction.execute("DELETE FROM oidc_pending WHERE attempt_hash = ?1", [&attempt])?;
            let session = open_browser_session(&transaction, account, false)?;
            transaction.commit()?;
            Ok((session, device_code))
        })
        .await
    }

    /// How many accounts sign in only with providers the server no longer has, or with
    /// passkeys made for another RP ID than `rp_id` (before `--auth-origin` changed): nothing but
    /// `be-server login-link --account ID` lets them in.
    pub async fn stranded_accounts(
        &self,
        issuers: Vec<String>,
        rp_id: Option<String>,
    ) -> Result<usize, ServerError> {
        self.with_database(move |_, database| {
            let mut query = database.prepare("SELECT account_id, kind, subject FROM identities")?;
            let mut usable = std::collections::HashMap::<String, bool>::new();
            for row in query.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })? {
                let (account, kind, subject) = row?;
                let works = signs_in(&kind, &subject, &issuers, rp_id.as_deref());
                *usable.entry(account).or_default() |= works;
            }
            Ok(usable.values().filter(|works| !**works).count())
        })
        .await
    }
}
