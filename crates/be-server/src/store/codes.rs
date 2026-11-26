//! Signing in, registering and proving it is you with a code mailed to the account's email
//! (guides/blocks/server.md, "Accounts").
//!
//! A code belongs to one pending attempt, named by a secret the requesting browser keeps in a
//! cookie (`be3-attempt`), never to the email alone: entering it anywhere else does nothing, and
//! a new request for the same attempt ends the older code. The mail's link fills the code in
//! only in the browser whose attempt it names; anywhere else it shows the code and signs nothing
//! in, so nobody can sign a victim's browser in to an attacker's account.
//!
//! Strangers can't spend the owner's mail: requests are capped per address (an IPv6 /64) and
//! per network (an IPv6 /48), and an account's own budget counts only requests from addresses
//! that never signed in to it (`Remembered::known`). A request for an email with no account does
//! the same work and sends nothing.

use super::*;
use crate::store::portal::now;

/// The letters a code is made of: no look-alikes (0 and O, 1, I and L, U and V).
pub const CODE_ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTWXYZ";
pub const CODE_LENGTH: usize = 8;
/// How long a code stays good.
pub const CODE_LIFETIME: i64 = 600;
/// Wrong tries a code survives; the next ends it.
pub const CODE_TRIES: i64 = 5;
/// The window the request limits count over, and how long a request is kept for them.
pub const CODE_WINDOW: i64 = 3600;
/// Requests one address (an IPv6 /64) may make in a window.
pub const CODES_PER_ADDRESS: i64 = 10;
/// Requests one network (an IPv6 /48) may make in a window, so one allocation can't spread
/// across many /64s.
pub const CODES_PER_NETWORK: i64 = 40;
/// Requests for one email in a window from addresses that never signed in to its account,
/// counted whether or not it has one. Large and quick to refill, since a stranger spending it
/// only delays the owner's codes from addresses the owner never used.
pub const CODES_PER_ACCOUNT: i64 = 20;
/// Registration codes outstanding at once, apart from every per-address cap: only holders of
/// an invite code can take one.
pub const MAX_REGISTRATION_CODES: i64 = 1024;
/// Registration codes one invite code may have waiting at once.
pub const REGISTRATION_CODES_PER_INVITE: i64 = 8;
/// How long a proof stays fresh for account changes.
pub const FRESH_PROOF: i64 = 600;

/// What a code is asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodePurpose {
    /// Signing in.
    SignIn,
    /// Signing in through "I can't sign in": every other session ends.
    Recover,
    /// Making an account.
    Register,
    /// Proving it is you again, for an account change.
    Proof,
    /// Signing in to the account that holds a provider's verified email, and linking that
    /// provider to it (`providers.rs`).
    Link,
}

impl CodePurpose {
    fn name(self) -> &'static str {
        match self {
            CodePurpose::SignIn => "sign-in",
            CodePurpose::Recover => "recover",
            CodePurpose::Register => "register",
            CodePurpose::Proof => "proof",
            CodePurpose::Link => "link",
        }
    }

    fn of(name: &str) -> Option<Self> {
        Some(match name {
            "sign-in" => CodePurpose::SignIn,
            "recover" => CodePurpose::Recover,
            "register" => CodePurpose::Register,
            "proof" => CodePurpose::Proof,
            "link" => CodePurpose::Link,
            _ => return None,
        })
    }
}

/// The purpose of a request for an email with no account: it costs what a real one does and
/// is never sent.
const UNSENT: &str = "unsent";

/// A code request.
pub struct CodeRequest<'a> {
    pub purpose: CodePurpose,
    /// The email typed, or for a proof the account's own.
    pub email: &'a str,
    /// For a proof, the account asking.
    pub account: Option<Uuid>,
    pub display_name: &'a str,
    pub invite: &'a str,
    /// The browser sign-in code to return to once signed in.
    pub device_code: &'a str,
    /// The requesting browser's attempt secret.
    pub attempt: &'a str,
    pub address: &'a str,
}

/// A code to mail.
pub struct MailedCode {
    pub email: String,
    pub purpose: CodePurpose,
    pub code: String,
    /// The public name of the attempt, which the mail's link carries.
    pub attempt: String,
}

/// What entering a right code did.
pub struct EnteredCode {
    pub purpose: CodePurpose,
    pub account: Uuid,
    /// A new browser session, for every purpose but a proof.
    pub session: Option<String>,
    pub device_code: String,
    /// The address the code was sent to.
    pub email: String,
    /// For a link, the provider identity it added, which the address is told of.
    pub linked: Option<String>,
    /// For a recovery, the ways to sign in it removed (`recover_account`).
    pub removed: Vec<Identity>,
}

/// An identity an account signs in with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub id: String,
    /// `email`, `oidc` or `passkey`.
    pub kind: String,
    /// The email, or for a provider `"{issuer} {sub}"`.
    pub subject: String,
    /// For a provider authoritative for email, the verified email it last gave; else empty.
    pub email: String,
    /// When it was added, in seconds since the epoch.
    pub created: i64,
}

/// The name of an attempt the mail's link carries: enough to tell the browser that began it,
/// and no use without that browser's secret.
pub fn public_attempt(attempt: &str) -> String {
    hash_token(TokenKind::PublicAttempt, attempt)[..24].to_owned()
}

/// What the network limit counts by: an IPv4 address, or an IPv6 address's /48.
fn network_key(address: &str) -> String {
    match address
        .parse::<std::net::IpAddr>()
        .map(|address| address.to_canonical())
    {
        Ok(std::net::IpAddr::V6(v6)) => {
            let segments = v6.segments();
            format!("{:x}:{:x}:{:x}::/48", segments[0], segments[1], segments[2])
        }
        Ok(v4) => v4.to_string(),
        Err(_) => address.to_owned(),
    }
}

fn new_code() -> String {
    let mut bytes = [0u8; CODE_LENGTH];
    // Only bytes below a multiple of the alphabet's length, so every letter is as likely.
    let limit = (256 / CODE_ALPHABET.len() * CODE_ALPHABET.len()) as u8;
    let mut code = String::with_capacity(CODE_LENGTH);
    while code.len() < CODE_LENGTH {
        rand::rngs::OsRng
            .try_fill_bytes(&mut bytes)
            .expect("the operating system has entropy");
        for byte in bytes.iter().filter(|byte| **byte < limit) {
            if code.len() < CODE_LENGTH {
                code.push(CODE_ALPHABET[*byte as usize % CODE_ALPHABET.len()] as char);
            }
        }
    }
    code
}

/// Whether `code` could be a code: `CODE_LENGTH` letters of `CODE_ALPHABET`.
pub fn is_well_formed(code: &str) -> bool {
    code.len() == CODE_LENGTH && code.bytes().all(|byte| CODE_ALPHABET.contains(&byte))
}

/// A code as typed: spaces and dashes go, letters are upper case.
pub fn normalize_code(code: &str) -> String {
    code.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_ascii_uppercase()
}

fn code_hash(attempt: &str, code: &str) -> String {
    hash_token(
        TokenKind::Code,
        &format!("{attempt}:{}", normalize_code(code)),
    )
}

fn refused(message: &str) -> ServerError {
    ServerError::Refused(ErrorCode::InvalidRequest, message.into())
}

fn too_many() -> ServerError {
    refused("Too many code requests. Try again in a while.")
}

/// The account an email identity belongs to.
pub(crate) fn account_of(
    database: &Connection,
    email: &str,
) -> Result<Option<String>, ServerError> {
    Ok(database
        .query_row(
            "SELECT account_id FROM identities WHERE kind = 'email' AND subject = ?1",
            [email],
            |row| row.get(0),
        )
        .optional()?)
}

/// Refuses an email an account already has: as an identity, or as the address an agent was
/// made with, which no one signs in with.
pub(crate) fn unregistered(database: &Connection, email: &str) -> Result<(), ServerError> {
    let used: bool = database.query_row(
        "SELECT EXISTS(SELECT 1 FROM identities WHERE kind = 'email' AND subject = ?1)
             OR EXISTS(SELECT 1 FROM accounts WHERE email = ?1)",
        [email],
        |row| row.get(0),
    )?;
    if used {
        return Err(refused("Email: that address is already registered."));
    }
    Ok(())
}

/// Makes an account, an editor of the personal organization, with `email` as its verified
/// email and an identity, if it has one.
pub(crate) fn make_account(
    database: &Connection,
    email: Option<&str>,
    name: &str,
) -> Result<Uuid, ServerError> {
    let account = Uuid::new_v4();
    database.execute(
        "INSERT INTO accounts (id, email, display_name) VALUES (?1, ?2, ?3)",
        params![account.to_string(), email, name],
    )?;
    if let Some(email) = email {
        add_email_identity(database, &account.to_string(), email)?;
    }
    let personal = personal_organization(database)?;
    database.execute(
        "INSERT INTO memberships (organization_id, account_id, role) VALUES (?1, ?2, 'editor')",
        params![personal.to_string(), account.to_string()],
    )?;
    Ok(account)
}

/// Takes back an account that may be in someone else's hands, before the recovering browser's
/// session opens: ends every session and pending email change, and the sessions of every agent
/// it made (which `delete_agents_created_by` then deletes), and removes every way to sign in
/// but `proven`, the email the recovery just proved, since any of them may be an intruder's
/// (guides/blocks/server.md, "Accounts"). With none proven, as for an operator's link to an account, the
/// account's email goes too, and its owner sets one again. Answers the ways it removed, for the
/// notice. The caller then calls `sessions_changed`.
pub(crate) fn recover_account(
    database: &Connection,
    account: Uuid,
    proven: Option<&str>,
) -> Result<Vec<Identity>, ServerError> {
    let account = account.to_string();
    let proven = proven.unwrap_or_default();
    database.execute("DELETE FROM sessions WHERE account_id = ?1", [&account])?;
    database.execute("DELETE FROM account_mail WHERE account_id = ?1", [&account])?;
    // In this transaction, so an agent's token stops at once even if deleting it fails later.
    database.execute(
        "DELETE FROM sessions WHERE account_id IN (SELECT account_id FROM agents WHERE created_by = ?1)",
        [&account],
    )?;
    let removed = database
        .prepare(
            "SELECT id, kind, subject, email, created FROM identities
             WHERE account_id = ?1 AND NOT (kind = 'email' AND subject = ?2) ORDER BY created, id",
        )?
        .query_map(params![account, proven], |row| {
            Ok(Identity {
                id: row.get(0)?,
                kind: row.get(1)?,
                subject: row.get(2)?,
                email: row.get(3)?,
                created: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    database.execute(
        "DELETE FROM identities WHERE account_id = ?1 AND NOT (kind = 'email' AND subject = ?2)",
        params![account, proven],
    )?;
    database.execute(
        "UPDATE accounts SET email = NULL WHERE id = ?1 AND email IS NOT ?2",
        params![account, (!proven.is_empty()).then_some(proven)],
    )?;
    Ok(removed)
}

/// Starts a browser session for the account that counts as fresh proof.
pub(crate) fn open_browser_session(
    database: &Connection,
    account: Uuid,
) -> Result<String, ServerError> {
    let now = now();
    let token = issue_token(database, account)?;
    database.execute(
        "UPDATE sessions SET kind = 'browser', expires = ?2, proved = ?3 WHERE token_hash = ?1",
        params![
            hash_token(TokenKind::Session, &token),
            now + 30 * 86400,
            now
        ],
    )?;
    Ok(token)
}

/// Adds an email identity to an account, as registering or verifying an email does.
pub(crate) fn add_email_identity(
    database: &Connection,
    account: &str,
    email: &str,
) -> Result<(), ServerError> {
    database.execute(
        "INSERT INTO identities (id, account_id, kind, subject, created) VALUES (?1, ?2, 'email', ?3, ?4)",
        params![Uuid::new_v4().to_string(), account, email, now()],
    )?;
    Ok(())
}

impl ServerStore {
    /// Asks for a code. The answer is a code to mail, or none for an email with no account,
    /// with the same limits and work either way; what the limits refuse doesn't depend on the
    /// email but for registering, which refuses an email that has an account
    /// (guides/blocks/server.md, Accounts).
    pub async fn request_code(
        &self,
        request: CodeRequest<'_>,
    ) -> Result<Option<MailedCode>, ServerError> {
        self.request_code_with(request, String::new(), String::new())
            .await
    }

    /// `request_code`, for a link code with the provider identity (`kind subject`) it links and
    /// the email the provider gave.
    pub(crate) async fn request_code_with(
        &self,
        request: CodeRequest<'_>,
        identity: String,
        identity_email: String,
    ) -> Result<Option<MailedCode>, ServerError> {
        let purpose = request.purpose;
        let email = match purpose {
            CodePurpose::Proof => request.email.to_owned(),
            _ => normalize_email(request.email)?,
        };
        if purpose == CodePurpose::Register {
            portal::validate_name(request.display_name)?;
        }
        let invite = invite_codes::invite_hash(request.invite);
        let address = crate::address::address_key_text(request.address);
        let network = network_key(request.address);
        let attempt = hash_token(TokenKind::Attempt, request.attempt);
        let name = request.display_name.trim().to_owned();
        let device_code = request.device_code.to_owned();
        let asking = request.account;
        let public = public_attempt(request.attempt);
        let known_email = self.remembered.known(&email, request.address);
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let now = now();
            transaction.execute("DELETE FROM sign_in_codes WHERE created <= ?1", [now - CODE_WINDOW])?;
            let count = |query: &str, value: &str| -> Result<i64, ServerError> {
                Ok(transaction.query_row(query, params![value, now - CODE_WINDOW], |row| row.get(0))?)
            };
            if count("SELECT count(*) FROM sign_in_codes WHERE address = ?1 AND created > ?2", &address)? >= CODES_PER_ADDRESS
                || count("SELECT count(*) FROM sign_in_codes WHERE network = ?1 AND created > ?2", &network)? >= CODES_PER_NETWORK
            {
                return Err(too_many());
            }
            let account = match purpose {
                CodePurpose::Proof => asking.map(|account| account.to_string()),
                CodePurpose::Register => {
                    invite_codes::check(&transaction, &invite)?;
                    unregistered(&transaction, &email)?;
                    let waiting: i64 = transaction.query_row(
                        "SELECT count(*) FROM sign_in_codes WHERE purpose = 'register' AND invite_hash = ?1 AND expires > ?2",
                        params![invite, now],
                        |row| row.get(0),
                    )?;
                    if waiting >= REGISTRATION_CODES_PER_INVITE {
                        return Err(refused(
                            "Invite code: too many registrations with this code are waiting on their email. Try again after they expire.",
                        ));
                    }
                    let registering: i64 = transaction.query_row(
                        "SELECT count(*) FROM sign_in_codes WHERE purpose = 'register' AND expires > ?1",
                        [now],
                        |row| row.get(0),
                    )?;
                    if registering >= MAX_REGISTRATION_CODES {
                        return Err(too_many());
                    }
                    None
                }
                CodePurpose::SignIn | CodePurpose::Recover | CodePurpose::Link => {
                    account_of(&transaction, &email)?
                }
            };
            let sent = match purpose {
                CodePurpose::Register => true,
                _ => account.is_some(),
            };
            // The email's budget counts strangers' requests whether or not it has an account, so
            // running it out tells nobody which emails do.
            if purpose != CodePurpose::Register && !known_email {
                let asked: i64 = transaction.query_row(
                    "SELECT count(*) FROM sign_in_codes WHERE email = ?1 AND remembered = 0 AND purpose != 'register' AND created > ?2",
                    params![email, now - CODE_WINDOW],
                    |row| row.get(0),
                )?;
                if asked >= CODES_PER_ACCOUNT {
                    return Err(too_many());
                }
            }
            // A new request for the same attempt ends the older code; its row still counts.
            transaction.execute(
                "UPDATE sign_in_codes SET expires = 0, attempt_hash = attempt_hash || ':' || rowid WHERE attempt_hash = ?1",
                [&attempt],
            )?;
            let code = new_code();
            transaction.execute(
                "INSERT INTO sign_in_codes (attempt_hash, code_hash, purpose, account_id, email, display_name, invite_hash, device_code, expires, created, address, network, remembered, identity, identity_email) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    attempt,
                    code_hash(&attempt, &code),
                    if sent { purpose.name() } else { UNSENT },
                    account.clone().unwrap_or_default(),
                    email,
                    name,
                    invite,
                    device_code,
                    now + CODE_LIFETIME,
                    now,
                    address,
                    network,
                    known_email,
                    identity,
                    identity_email,
                ],
            )?;
            transaction.commit()?;
            Ok(sent.then_some(MailedCode {
                email,
                purpose,
                code,
                attempt: public,
            }))
        })
        .await
    }

    /// Whether `attempt` is the attempt the mail's link names.
    pub fn is_attempt(attempt: &str, public: &str) -> bool {
        !attempt.is_empty() && public_attempt(attempt) == public
    }

    /// The email a pending attempt's code went to, for the page that asks for it.
    pub async fn attempt_email(&self, attempt: &str) -> Option<String> {
        let attempt = hash_token(TokenKind::Attempt, attempt);
        self.with_database(move |_, database| {
            Ok(database
                .query_row(
                    "SELECT email FROM sign_in_codes WHERE attempt_hash = ?1 AND expires > ?2",
                    params![attempt, now()],
                    |row| row.get(0),
                )
                .optional()?)
        })
        .await
        .ok()
        .flatten()
    }

    /// Enters a code for the browser's attempt. A wrong code counts a try; the code dies at
    /// the last. `browser` is the browser's session, which a proof is for.
    /// `allowed` says whether the server takes the purpose now: signing in, or registering.
    pub async fn enter_code(
        &self,
        attempt: &str,
        code: &str,
        browser: &str,
        address: &str,
        allowed: fn(CodePurpose, &crate::ServerConfig) -> bool,
        config: crate::ServerConfig,
    ) -> Result<EnteredCode, ServerError> {
        let attempt_hash = hash_token(TokenKind::Attempt, attempt);
        let entered = code_hash(&attempt_hash, code);
        let browser = hash_token(TokenKind::Session, browser);
        let entered_code = self
            .with_database(move |store, database| {
                let transaction = be_block_sqlite_schema::write_transaction(database)?;
                let now = now();
                #[allow(clippy::type_complexity)]
                let row: Option<(String, String, String, String, String, String, String, i64, String, String)> = transaction
                    .query_row(
                        "SELECT code_hash, purpose, account_id, email, display_name, invite_hash, device_code, tries, identity, identity_email FROM sign_in_codes WHERE attempt_hash = ?1 AND expires > ?2",
                        params![attempt_hash, now],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?)),
                    )
                    .optional()?;
                let Some((hash, purpose, account, email, name, invite, device_code, tries, identity, identity_email)) = row else {
                    return Err(refused("Code: this code has expired or was replaced. Ask for a new one."));
                };
                if let Some(purpose) = CodePurpose::of(&purpose)
                    && !allowed(purpose, &config)
                {
                    return Err(refused(match purpose {
                        CodePurpose::Register => "This server is not accepting new accounts.",
                        _ => "This server is not accepting sign-ins.",
                    }));
                }
                let purpose = CodePurpose::of(&purpose).filter(|_| hash == entered);
                let Some(purpose) = purpose else {
                    let tries = tries + 1;
                    transaction.execute(
                        "UPDATE sign_in_codes SET tries = ?2, expires = CASE WHEN ?2 >= ?3 THEN 0 ELSE expires END WHERE attempt_hash = ?1",
                        params![attempt_hash, tries, CODE_TRIES],
                    )?;
                    transaction.commit()?;
                    return Err(refused(&match tries >= CODE_TRIES {
                        true => "Code: too many wrong tries. Ask for a new code.".to_owned(),
                        false => "Code: that code is not right. Check the latest email and try again.".to_owned(),
                    }));
                };
                transaction.execute(
                    "UPDATE sign_in_codes SET expires = 0, attempt_hash = attempt_hash || ':' || rowid WHERE attempt_hash = ?1",
                    [&attempt_hash],
                )?;
                let account = match purpose {
                    CodePurpose::Register => {
                        unregistered(&transaction, &email)?;
                        invite_codes::spend(&transaction, &invite)?;
                        make_account(&transaction, Some(&email), &name)?
                    }
                    CodePurpose::Link => {
                        let account = parse_uuid(&account)?;
                        super::providers::add_provider_identity(&transaction, account, &identity, &identity_email)?;
                        account
                    }
                    _ => parse_uuid(&account)?,
                };
                let removed = match purpose {
                    CodePurpose::Recover => recover_account(&transaction, account, Some(&email))?,
                    _ => Vec::new(),
                };
                let session = match purpose {
                    CodePurpose::Proof => {
                        note_proof(&transaction, &browser, account)?;
                        None
                    }
                    _ => Some(open_browser_session(&transaction, account)?),
                };
                transaction.commit()?;
                if purpose == CodePurpose::Recover {
                    store.sessions_changed();
                }
                let linked = (purpose == CodePurpose::Link).then_some(identity);
                Ok((EnteredCode { purpose, account, session, device_code, email: email.clone(), linked, removed }, email))
            })
            .await;
        let (entered, email) = entered_code?;
        self.remembered.remember(&email, address, Instant::now());
        Ok(entered)
    }

    /// Asks for a code and enters it at once, as a browser at `request.address` would, for the
    /// tools and tests that hold the store rather than a mailbox; the portal never calls it.
    /// `request.attempt` is ignored: each call is an attempt of its own. Answers what entering
    /// it did, or why it was refused, a code for an email with no account included.
    pub async fn ask_and_enter_code(
        &self,
        request: CodeRequest<'_>,
        config: crate::ServerConfig,
    ) -> Result<EnteredCode, ServerError> {
        let attempt = crate::new_secret();
        let address = request.address;
        let mailed = self
            .request_code(CodeRequest {
                attempt: &attempt,
                ..request
            })
            .await?
            .ok_or_else(|| refused("Email: no account has that address."))?;
        self.enter_code(&attempt, &mailed.code, "", address, |_, _| true, config)
            .await
    }

    /// Whether the browser session proved it is the account's owner in the last
    /// `FRESH_PROOF`: by a code to its email, or by signing in.
    pub async fn is_fresh(&self, browser: &str) -> Result<bool, ServerError> {
        let browser = hash_token(TokenKind::Session, browser);
        self.with_database(move |_, database| {
            Ok(database.query_row(
                "SELECT EXISTS(SELECT 1 FROM sessions WHERE token_hash = ?1 AND kind = 'browser' AND expires > ?2 AND proved > ?3)",
                params![browser, now(), now() - FRESH_PROOF],
                |row| row.get(0),
            )?)
        })
        .await
    }

    /// Refuses an account change unless the browser proved it is the owner just now.
    pub async fn require_fresh(&self, browser: &str) -> Result<(), ServerError> {
        match self.is_fresh(browser).await? {
            true => Ok(()),
            false => Err(refused(NOT_FRESH)),
        }
    }
}

const NOT_FRESH: &str =
    "Confirm it is you first, with a code to your email, a passkey or a provider you sign in with.";

/// Refuses unless the browser session hashed as `browser` is `account`'s and proved it is the
/// owner in the last `FRESH_PROOF`; checked in the transaction of the change it allows, so a
/// session that changed or went stale meanwhile changes nothing.
pub(crate) fn require_fresh_proof(
    database: &Connection,
    browser: &str,
    account: Uuid,
) -> Result<(), ServerError> {
    let fresh: bool = database.query_row(
        "SELECT EXISTS(SELECT 1 FROM sessions WHERE token_hash = ?1 AND account_id = ?2 AND kind = 'browser' AND expires > ?3 AND proved > ?4)",
        params![browser, account.to_string(), now(), now() - FRESH_PROOF],
        |row| row.get(0),
    )?;
    match fresh {
        true => Ok(()),
        false => Err(refused(NOT_FRESH)),
    }
}

/// Notes that the browser session hashed as `browser`, `account`'s, proved it is the owner just
/// now; refuses one that is not the account's or has ended.
pub(crate) fn note_proof(
    database: &Connection,
    browser: &str,
    account: Uuid,
) -> Result<(), ServerError> {
    let proved = database.execute(
        "UPDATE sessions SET proved = ?3 WHERE token_hash = ?1 AND account_id = ?2 AND kind = 'browser' AND expires > ?3",
        params![browser, account.to_string(), now()],
    )?;
    match proved {
        0 => Err(refused("Sign in to continue.")),
        _ => Ok(()),
    }
}

impl ServerStore {
    pub async fn identities(&self, account: Uuid) -> Result<Vec<Identity>, ServerError> {
        self.with_database(move |_, database| {
            let mut query = database.prepare(
                "SELECT id, kind, subject, email, created FROM identities WHERE account_id = ?1 ORDER BY created, id",
            )?;
            let found = query
                .query_map([account.to_string()], |row| {
                    Ok(Identity {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        subject: row.get(2)?,
                        email: row.get(3)?,
                        created: row.get(4)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(found)
        })
        .await
    }

    /// Removes one of an account's identities, never its last that still signs in
    /// (`providers::signs_in` with the server's `issuers` and the portal's `rp_id`), after a
    /// fresh proof. Answers the identity removed, for the notice.
    pub async fn remove_identity(
        &self,
        account: Uuid,
        browser: &str,
        id: &str,
        issuers: Vec<String>,
        rp_id: Option<String>,
    ) -> Result<Identity, ServerError> {
        self.require_fresh(browser).await?;
        let id = id.to_owned();
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let identity: Option<Identity> = transaction
                .query_row(
                    "SELECT id, kind, subject, email, created FROM identities WHERE id = ?1 AND account_id = ?2",
                    params![id, account.to_string()],
                    |row| {
                        Ok(Identity {
                            id: row.get(0)?,
                            kind: row.get(1)?,
                            subject: row.get(2)?,
                            email: row.get(3)?,
                            created: row.get(4)?,
                        })
                    },
                )
                .optional()?;
            let identity =
                identity.ok_or_else(|| refused("That way to sign in is not on your account."))?;
            let mut others = transaction.prepare(
                "SELECT kind, subject FROM identities WHERE account_id = ?1 AND id != ?2",
            )?;
            let mut usable = false;
            for row in others.query_map(params![account.to_string(), id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })? {
                let (kind, subject) = row?;
                usable |= super::providers::signs_in(&kind, &subject, &issuers, rp_id.as_deref());
            }
            drop(others);
            if !usable {
                return Err(refused(
                    "This is your only way to sign in, so it can't be removed. Add another first.",
                ));
            }
            transaction.execute("DELETE FROM identities WHERE id = ?1", [&id])?;
            // An email it no longer signs in with is no longer its email, nor its providers'.
            if identity.kind == "email" {
                transaction.execute(
                    "UPDATE accounts SET email = NULL WHERE id = ?1 AND email = ?2",
                    params![account.to_string(), identity.subject],
                )?;
                transaction.execute(
                    "UPDATE identities SET email = '' WHERE account_id = ?1 AND kind = 'oidc' AND email = ?2",
                    params![account.to_string(), identity.subject],
                )?;
            }
            transaction.commit()?;
            Ok(identity)
        })
        .await
    }
}
