//! Signing in, registering and proving it is you with a code mailed to the account's email
//! (plans/accounts.md, "Rules every method follows").
//!
//! A code belongs to one pending attempt, named by a secret the requesting browser keeps in a
//! cookie (`be3-attempt`), never to the email alone: entering it anywhere else does nothing, and
//! a new request for the same attempt ends the older code. The mail's link fills the code in
//! only in the browser whose attempt it names; anywhere else it shows the code and signs nothing
//! in, so nobody can sign a victim's browser in to an attacker's account.
//!
//! Strangers can't spend the owner's mail: requests are capped per address (an IPv6 /64) and
//! per network (an IPv6 /48), and an account's own budget counts only requests from addresses
//! that never signed in to it (`Passwords::known`). A request for an email with no account does
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
/// Requests for one account in a window from addresses that never signed in to it. Large and
/// quick to refill, since a stranger spending it only delays the owner's codes from addresses
/// the owner never used.
pub const CODES_PER_ACCOUNT: i64 = 20;
/// Registration codes outstanding at once, apart from every per-address cap, with the
/// registration and email-change links (`portal::MAX_MAIL_LINKS`): only holders of an invite
/// code can take one.
pub const MAX_REGISTRATION_CODES: i64 = 1024;
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
}

impl CodePurpose {
    fn name(self) -> &'static str {
        match self {
            CodePurpose::SignIn => "sign-in",
            CodePurpose::Recover => "recover",
            CodePurpose::Register => "register",
            CodePurpose::Proof => "proof",
        }
    }

    fn of(name: &str) -> Option<Self> {
        Some(match name {
            "sign-in" => CodePurpose::SignIn,
            "recover" => CodePurpose::Recover,
            "register" => CodePurpose::Register,
            "proof" => CodePurpose::Proof,
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
}

/// An identity an account signs in with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub id: String,
    /// `email`, and later `passkey` and `oidc`.
    pub kind: String,
    pub subject: String,
}

/// A fresh attempt secret for a browser's `be3-attempt` cookie.
pub fn new_attempt() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

/// The name of an attempt the mail's link carries: enough to tell the browser that began it,
/// and no use without that browser's secret.
pub fn public_attempt(attempt: &str) -> String {
    hash_token(&format!("attempt link {attempt}"))[..24].to_owned()
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
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .expect("the operating system has entropy");
    bytes
        .iter()
        .map(|byte| CODE_ALPHABET[*byte as usize % CODE_ALPHABET.len()] as char)
        .collect()
}

/// A code as typed: spaces and dashes go, letters are upper case.
pub fn normalize_code(code: &str) -> String {
    code.chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_ascii_uppercase()
}

fn code_hash(attempt: &str, code: &str) -> String {
    hash_token(&format!("{attempt}:{}", normalize_code(code)))
}

fn refused(message: &str) -> ServerError {
    ServerError::Refused(ErrorCode::InvalidRequest, message.into())
}

fn too_many() -> ServerError {
    refused("Too many code requests. Try again in a while.")
}

/// The account an email identity belongs to.
fn account_of(database: &Connection, email: &str) -> Result<Option<String>, ServerError> {
    Ok(database
        .query_row(
            "SELECT account_id FROM identities WHERE kind = 'email' AND subject = ?1",
            [email],
            |row| row.get(0),
        )
        .optional()?)
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
        let purpose = request.purpose;
        let email = match purpose {
            CodePurpose::Proof => request.email.to_owned(),
            _ => normalize_email(request.email)?,
        };
        if purpose == CodePurpose::Register {
            portal::validate_name(request.display_name)?;
        }
        let invite = invite_codes::invite_hash(request.invite);
        let address = passwords::address_key(request.address);
        let network = network_key(request.address);
        let attempt = hash_token(request.attempt);
        let name = request.display_name.trim().to_owned();
        let device_code = request.device_code.to_owned();
        let asking = request.account;
        let public = public_attempt(request.attempt);
        let known_email = self.passwords.known(&email, request.address);
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
                    if account_of(&transaction, &email)?.is_some() {
                        return Err(refused("Email: that address is already registered."));
                    }
                    let waiting: i64 = transaction.query_row(
                        "SELECT count(*) FROM sign_in_codes WHERE purpose = 'register' AND invite_hash = ?1 AND expires > ?2",
                        params![invite, now],
                        |row| row.get(0),
                    )?;
                    if waiting >= portal::MAX_MAIL_LINKS_PER_INVITE {
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
                CodePurpose::SignIn | CodePurpose::Recover => account_of(&transaction, &email)?,
            };
            let sent = match purpose {
                CodePurpose::Register => true,
                _ => account.is_some(),
            };
            if let Some(account) = account.as_deref().filter(|_| !known_email) {
                let asked: i64 = transaction.query_row(
                    "SELECT count(*) FROM sign_in_codes WHERE account_id = ?1 AND remembered = 0 AND created > ?2",
                    params![account, now - CODE_WINDOW],
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
                "INSERT INTO sign_in_codes (attempt_hash, code_hash, purpose, account_id, email, display_name, invite_hash, device_code, expires, created, address, network, remembered) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
                ],
            )?;
            transaction.commit()?;
            Ok(sent.then(|| MailedCode {
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
        let attempt = hash_token(attempt);
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
    pub async fn enter_code(
        &self,
        attempt: &str,
        code: &str,
        browser: &str,
        address: &str,
    ) -> Result<EnteredCode, ServerError> {
        let attempt_hash = hash_token(attempt);
        let entered = code_hash(&attempt_hash, code);
        let browser = hash_token(browser);
        let entered_code = self
            .with_database(move |store, database| {
                let transaction = be_block_sqlite_schema::write_transaction(database)?;
                let now = now();
                let row: Option<(String, String, String, String, String, String, String, i64)> = transaction
                    .query_row(
                        "SELECT code_hash, purpose, account_id, email, display_name, invite_hash, device_code, tries FROM sign_in_codes WHERE attempt_hash = ?1 AND expires > ?2",
                        params![attempt_hash, now],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?)),
                    )
                    .optional()?;
                let Some((hash, purpose, account, email, name, invite, device_code, tries)) = row else {
                    return Err(refused("Code: this code has expired or was replaced. Ask for a new one."));
                };
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
                        if account_of(&transaction, &email)?.is_some() {
                            return Err(refused("Email: that address is already registered."));
                        }
                        invite_codes::spend(&transaction, &invite)?;
                        let account = Uuid::new_v4();
                        transaction.execute(
                            "INSERT INTO accounts (id, email, display_name, password_hash) VALUES (?1, ?2, ?3, '')",
                            params![account.to_string(), email, name],
                        )?;
                        add_email_identity(&transaction, &account.to_string(), &email)?;
                        let personal = personal_organization(&transaction)?;
                        transaction.execute(
                            "INSERT INTO memberships (organization_id, account_id, role) VALUES (?1, ?2, 'editor')",
                            params![personal.to_string(), account.to_string()],
                        )?;
                        account
                    }
                    _ => parse_uuid(&account)?,
                };
                let session = match purpose {
                    CodePurpose::Proof => {
                        let proved = transaction.execute(
                            "UPDATE sessions SET proved = ?3 WHERE token_hash = ?1 AND account_id = ?2 AND kind = 'browser' AND expires > ?3",
                            params![browser, account.to_string(), now],
                        )?;
                        if proved == 0 {
                            return Err(refused("Sign in to continue."));
                        }
                        None
                    }
                    _ => {
                        if purpose == CodePurpose::Recover {
                            transaction.execute("DELETE FROM sessions WHERE account_id = ?1", [account.to_string()])?;
                            transaction.execute("DELETE FROM account_mail WHERE account_id = ?1", [account.to_string()])?;
                        }
                        let token = issue_token(&transaction, account)?;
                        transaction.execute(
                            "UPDATE sessions SET kind = 'browser', expires = ?2, proved = ?3 WHERE token_hash = ?1",
                            params![hash_token(&token), now + 30 * 86400, now],
                        )?;
                        Some(token)
                    }
                };
                transaction.commit()?;
                if purpose == CodePurpose::Recover {
                    store.sessions_changed();
                }
                Ok((EnteredCode { purpose, account, session, device_code }, email))
            })
            .await;
        let (entered, email) = entered_code?;
        self.passwords.remember(&email, address, Instant::now());
        Ok(entered)
    }

    /// Whether the browser session proved it is the account's owner in the last
    /// `FRESH_PROOF`: by a code to its email, or by signing in.
    pub async fn is_fresh(&self, browser: &str) -> Result<bool, ServerError> {
        let browser = hash_token(browser);
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
            false => Err(ServerError::Refused(
                ErrorCode::InvalidRequest,
                "Confirm it is you first: ask for a code to your email.".into(),
            )),
        }
    }

    pub async fn identities(&self, account: Uuid) -> Result<Vec<Identity>, ServerError> {
        self.with_database(move |_, database| {
            let mut query = database.prepare(
                "SELECT id, kind, subject FROM identities WHERE account_id = ?1 ORDER BY created, id",
            )?;
            let found = query
                .query_map([account.to_string()], |row| {
                    Ok(Identity {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        subject: row.get(2)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(found)
        })
        .await
    }

    /// Removes one of an account's identities, never its last, after a fresh proof. Answers
    /// the identity removed, for the notice.
    pub async fn remove_identity(
        &self,
        account: Uuid,
        browser: &str,
        id: &str,
    ) -> Result<Identity, ServerError> {
        self.require_fresh(browser).await?;
        let id = id.to_owned();
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let identity: Option<Identity> = transaction
                .query_row(
                    "SELECT id, kind, subject FROM identities WHERE id = ?1 AND account_id = ?2",
                    params![id, account.to_string()],
                    |row| {
                        Ok(Identity {
                            id: row.get(0)?,
                            kind: row.get(1)?,
                            subject: row.get(2)?,
                        })
                    },
                )
                .optional()?;
            let identity =
                identity.ok_or_else(|| refused("That way to sign in is not on your account."))?;
            let others: i64 = transaction.query_row(
                "SELECT count(*) FROM identities WHERE account_id = ?1 AND id != ?2",
                params![account.to_string(), id],
                |row| row.get(0),
            )?;
            if others == 0 {
                return Err(refused(
                    "This is your only way to sign in, so it can't be removed. Add another first.",
                ));
            }
            transaction.execute("DELETE FROM identities WHERE id = ?1", [&id])?;
            transaction.commit()?;
            Ok(identity)
        })
        .await
    }
}
