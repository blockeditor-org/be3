use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

/// Registration and email-change links outstanding at once. Only those who
/// hold an invite code or are signed in can take one, so anonymous traffic
/// can't fill it; resets have no global cap, being at most
/// `MAX_MAIL_LINKS_PER_EMAIL` for each account.
pub const MAX_MAIL_LINKS: i64 = 1024;
/// Mail requests one address (an IPv6 /64) may have outstanding, sent or not.
pub const MAX_MAIL_LINKS_PER_ADDRESS: i64 = 8;
/// Mail links one email may have outstanding for one purpose.
pub const MAX_MAIL_LINKS_PER_EMAIL: i64 = 3;
/// Registration links one invite code may have waiting on their email.
pub const MAX_MAIL_LINKS_PER_INVITE: i64 = 8;
/// Rows reset requests for unknown emails may leave, apart from every
/// other cap; once full, such requests leave none.
pub const MAX_UNSENT_RESETS: i64 = 1024;
const REGISTRATION_LINK_LIFETIME: i64 = 3600;
const RESET_LINK_LIFETIME: i64 = 1800;
/// The purpose of a row a reset request for an unknown email leaves: it costs
/// the requester what a real reset would and is never sent.
const UNSENT: &str = "unsent";

pub struct AccountSession {
    pub id: String,
    pub kind: String,
    pub created: i64,
    pub current: bool,
}

pub struct MailLink {
    pub email: String,
    pub purpose: String,
    pub token: String,
}

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn refused(message: &str) -> ServerError {
    ServerError::Refused(ErrorCode::InvalidRequest, message.into())
}

pub(crate) fn validate_password(password: &str) -> Result<(), ServerError> {
    if password.chars().count() < 8 || password.len() > 1024 {
        return Err(refused(
            "Password: use at least 8 characters and at most 1024 bytes.",
        ));
    }
    Ok(())
}

pub(crate) fn validate_name(name: &str) -> Result<(), ServerError> {
    if name.trim().is_empty() || name.len() > 200 {
        return Err(refused("Display name: enter a name of at most 200 bytes."));
    }
    Ok(())
}

impl ServerStore {
    pub(crate) fn sessions_changed(&self) {
        self.session_changes
            .send_modify(|version| *version = version.wrapping_add(1));
    }

    pub async fn claim_browser_session(
        &self,
        account: Uuid,
        issuer: &str,
    ) -> Result<String, ServerError> {
        let issuer = issuer.to_owned();
        self.with_database(move |_, database| {
            let transaction = database.unchecked_transaction()?;
            let valid: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE account_id = ?1 AND token_hash = ?2 AND kind = 'browser' AND expires > ?3)", params![account.to_string(), hash_token(&issuer), now()], |row| row.get(0))?;
            if !valid {
                return Err(refused(
                    "The approving browser session expired or was revoked. Start sign-in again.",
                ));
            }
            let token = issue_token(&transaction, account)?;
            transaction.commit()?;
            Ok(token)
        })
        .await
    }

    pub async fn browser_session(&self, account: Uuid) -> Result<String, ServerError> {
        self.with_database(move |_, database| {
            let token = issue_token(&database, account)?;
            database.execute(
                "UPDATE sessions SET kind = 'browser', expires = ?2 WHERE token_hash = ?1",
                params![hash_token(&token), now() + 30 * 86400],
            )?;
            Ok(token)
        })
        .await
    }

    pub async fn browser_profile(&self, token: &str) -> Result<Profile, ServerError> {
        let token = token.to_owned();
        self.with_database(move |_, database| {
            let row: Option<(String, String, String)> = database.query_row(
                "SELECT a.id, a.email, a.display_name FROM sessions s JOIN accounts a ON a.id = s.account_id WHERE s.token_hash = ?1 AND s.kind = 'browser' AND s.expires > ?2",
                params![hash_token(&token), now()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).optional()?;
            let (account, email, display_name) = row.ok_or_else(|| refused("Sign in to continue."))?;
            Ok(Profile {
                account: parse_uuid(&account)?,
                email,
                display_name,
            })
        })
        .await
    }

    pub async fn account_sessions(
        &self,
        account: Uuid,
        current: &str,
    ) -> Result<Vec<AccountSession>, ServerError> {
        let current = current.to_owned();
        self.with_database(move |_, database| {
            let mut query = database.prepare("SELECT public_id, kind, created, token_hash FROM sessions WHERE account_id = ?1 AND (kind = 'app' OR (kind = 'browser' AND expires > ?2)) ORDER BY created DESC, public_id")?;
            let sessions = query
                .query_map(params![account.to_string(), now()], |row| {
                    Ok(AccountSession {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        created: row.get(2)?,
                        current: row.get::<_, String>(3)? == hash_token(&current),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(sessions)
        })
        .await
    }

    pub async fn revoke_session(&self, account: Uuid, id: &str) -> Result<(), ServerError> {
        let id = id.to_owned();
        let deleted = self
            .with_database(move |_, database| {
                Ok(database.execute(
                    "DELETE FROM sessions WHERE account_id = ?1 AND public_id = ?2",
                    params![account.to_string(), id],
                )?)
            })
            .await?;
        if deleted > 0 {
            self.sessions_changed();
        }
        Ok(())
    }

    pub async fn revoke_other_sessions(
        &self,
        account: Uuid,
        current: &str,
    ) -> Result<(), ServerError> {
        let current = hash_token(current);
        let deleted = self
            .with_database(move |_, database| {
                Ok(database.execute(
                    "DELETE FROM sessions WHERE account_id = ?1 AND token_hash != ?2",
                    params![account.to_string(), current],
                )?)
            })
            .await?;
        if deleted > 0 {
            self.sessions_changed();
        }
        Ok(())
    }

    pub async fn update_display_name(&self, account: Uuid, name: &str) -> Result<(), ServerError> {
        validate_name(name)?;
        let name = name.trim().to_owned();
        self.with_database(move |_, database| {
            database.execute(
                "UPDATE accounts SET display_name = ?2 WHERE id = ?1",
                params![account.to_string(), name],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn check_password(
        &self,
        account: Uuid,
        password: &str,
        address: &str,
    ) -> Result<String, ServerError> {
        let profile = self.profile(account).await?;
        if let Some(wait) = self
            .passwords
            .locked_out(&profile.email, address, true, Instant::now())
        {
            return Err(refused(&format!(
                "Too many failed attempts. Try again in {} seconds.",
                wait.as_secs().max(1)
            )));
        }
        let hash: String = self
            .with_database(move |_, database| {
                Ok(database.query_row(
                    "SELECT password_hash FROM accounts WHERE id = ?1 AND local_profile = 0",
                    [account.to_string()],
                    |row| row.get(0),
                )?)
            })
            .await?;
        if !self.verify(password.into(), hash.clone(), address).await? {
            self.passwords
                .failed(&profile.email, address, Instant::now());
            return Err(refused("Current password: incorrect password."));
        }
        self.passwords
            .succeeded(&profile.email, address, Instant::now());
        Ok(hash)
    }

    pub async fn change_password(
        &self,
        account: Uuid,
        current: &str,
        password: &str,
        browser_token: &str,
        address: &str,
    ) -> Result<String, ServerError> {
        validate_password(password)?;
        let previous_hash = self.check_password(account, current, address).await?;
        let hash = self.hash(password.into(), address).await?;
        let browser_token = browser_token.to_owned();
        self.with_database(move |store, database| {
            let transaction = database.unchecked_transaction()?;
            let valid: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE account_id = ?1 AND token_hash = ?2 AND kind = 'browser' AND expires > ?3 AND EXISTS(SELECT 1 FROM accounts WHERE id = ?1 AND password_hash = ?4))", params![account.to_string(), hash_token(&browser_token), now(), previous_hash], |row| row.get(0))?;
            if !valid {
                return Err(refused("Sign in again to change your password."));
            }
            transaction.execute(
                "UPDATE accounts SET password_hash = ?2 WHERE id = ?1",
                params![account.to_string(), hash],
            )?;
            transaction.execute(
                "DELETE FROM sessions WHERE account_id = ?1",
                [account.to_string()],
            )?;
            transaction.execute(
                "DELETE FROM account_mail WHERE account_id = ?1",
                [account.to_string()],
            )?;
            let token = issue_token(&transaction, account)?;
            transaction.execute(
                "UPDATE sessions SET kind = 'browser', expires = ?2 WHERE token_hash = ?1",
                params![hash_token(&token), now() + 30 * 86400],
            )?;
            transaction.commit()?;
            store.sessions_changed();
            Ok(token)
        })
        .await
    }

    /// Starts registering: every cheap check (the fields, the invite code,
    /// the mail-link limits, whether the email is taken) comes before the
    /// password is hashed.
    #[allow(clippy::too_many_arguments)]
    pub async fn registration_link(
        &self,
        email: &str,
        name: &str,
        password: &str,
        invite: &str,
        code: &str,
        address: &str,
    ) -> Result<MailLink, ServerError> {
        let email = normalize_email(email)?;
        validate_name(name)?;
        validate_password(password)?;
        let invite = invite_codes::invite_hash(invite);
        let address = passwords::address_key(address);
        {
            let (invite, email, address) = (invite.clone(), email.clone(), address.clone());
            self.with_database(move |_, database| {
                invite_codes::check(database, &invite)?;
                may_send(database, "register", &email, &address)?;
                may_send_with(database, &invite)?;
                unregistered(database, &email)
            })
            .await?;
        }
        let hash = self.hash(password.into(), &address).await?;
        let name = name.to_owned();
        let code = code.to_owned();
        self.with_database(move |_, database| {
            let transaction = database.unchecked_transaction()?;
            invite_codes::check(&transaction, &invite)?;
            let link = mail_link(
                &transaction,
                Pending {
                    purpose: "register",
                    account: "",
                    email: &email,
                    name: name.trim(),
                    password_hash: &hash,
                    code: &code,
                    invite: &invite,
                    address: &address,
                },
            )?;
            transaction.commit()?;
            Ok(link)
        })
        .await
    }

    pub async fn email_change_link(
        &self,
        account: Uuid,
        email: &str,
        password: &str,
        address: &str,
    ) -> Result<MailLink, ServerError> {
        let email = normalize_email(email)?;
        let address = passwords::address_key(address);
        {
            let (email, address) = (email.clone(), address.clone());
            self.with_database(move |_, database| may_send(database, "email", &email, &address))
                .await?;
        }
        let previous_hash = self.check_password(account, password, &address).await?;
        self.with_database(move |_, database| {
            let transaction = database.unchecked_transaction()?;
            let link = mail_link(
                &transaction,
                Pending {
                    purpose: "email",
                    account: &account.to_string(),
                    email: &email,
                    name: "",
                    password_hash: &previous_hash,
                    code: "",
                    invite: "",
                    address: &address,
                },
            )?;
            transaction.commit()?;
            Ok(link)
        })
        .await
    }

    /// A reset link for `email` if it has an account. The answer and the
    /// work are the same whether it has one or not: an unknown email leaves
    /// an unsent row that counts against the address like a real link (but
    /// against no global cap), and only the limit that doesn't depend on the
    /// email, the address's, is shown.
    pub async fn password_reset_link(
        &self,
        email: &str,
        code: &str,
        address: &str,
    ) -> Result<Option<MailLink>, ServerError> {
        let Ok(email) = normalize_email(email) else {
            return Ok(None);
        };
        let address = passwords::address_key(address);
        let code = code.to_owned();
        self.with_database(move |_, database| {
            let transaction = database.unchecked_transaction()?;
            may_send_from(&transaction, "reset", &address)?;
            let account: Option<String> = transaction
                .query_row(
                    "SELECT id FROM accounts WHERE email = ?1 AND local_profile = 0",
                    [&email],
                    |row| row.get(0),
                )
                .optional()?;
            let link = mail_link(
                &transaction,
                Pending {
                    purpose: if account.is_some() { "reset" } else { UNSENT },
                    account: account.as_deref().unwrap_or_default(),
                    email: &email,
                    name: "",
                    password_hash: "",
                    code: &code,
                    invite: "",
                    address: &address,
                },
            );
            let link = match link {
                Ok(link) => link,
                Err(ServerError::Refused(..)) => return Ok(None),
                Err(error) => return Err(error),
            };
            transaction.commit()?;
            Ok(account.map(|_| link))
        })
        .await
    }

    pub async fn mail_purpose(&self, token: &str) -> Result<String, ServerError> {
        let token = token.to_owned();
        self.with_database(move |_, database| {
            database
                .query_row(
                    "SELECT purpose FROM account_mail WHERE token_hash = ?1 AND expires > ?2 AND purpose != ?3",
                    params![hash_token(&token), now(), UNSENT],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or_else(|| refused("This link is invalid, expired, or already used."))
        })
        .await
    }

    pub async fn confirm_mail(
        &self,
        token: &str,
        password: Option<&str>,
        address: &str,
    ) -> Result<(Option<Uuid>, String, Option<String>), ServerError> {
        let new_hash = match password {
            Some(password) => {
                validate_password(password)?;
                self.mail_purpose(token).await?;
                Some(self.hash(password.into(), address).await?)
            }
            None => None,
        };
        let token = token.to_owned();
        let address = address.to_owned();
        self.with_database(move |store, database| {
            let transaction = database.unchecked_transaction()?;
            let row: Option<(String, String, String, String, String, String, String)> = transaction.query_row("SELECT purpose, account_id, email, display_name, password_hash, device_code, invite_hash FROM account_mail WHERE token_hash = ?1 AND expires > ?2", params![hash_token(&token), now()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?))).optional()?;
            let (purpose, account, email, name, hash, code, invite) = row.ok_or_else(|| {
                refused("This link is invalid, expired, or already used. Request a new link.")
            })?;
            let mut old_email = None;
            let result = match purpose.as_str() {
                "register" if new_hash.is_none() => {
                    let used: bool = transaction.query_row(
                        "SELECT EXISTS(SELECT 1 FROM accounts WHERE email = ?1)",
                        [&email],
                        |row| row.get(0),
                    )?;
                    if used {
                        return Err(refused("Email: that address is already registered."));
                    }
                    invite_codes::spend(&transaction, &invite)?;
                    let account = Uuid::new_v4();
                    transaction.execute("INSERT INTO accounts (id, email, display_name, password_hash) VALUES (?1, ?2, ?3, ?4)", params![account.to_string(), email, name, hash])?;
                    let personal = personal_organization(&transaction)?;
                    transaction.execute("INSERT INTO memberships (organization_id, account_id, role) VALUES (?1, ?2, 'editor')", params![personal.to_string(), account.to_string()])?;
                    Some(account)
                }
                "email" if new_hash.is_none() => {
                    let unchanged: bool = transaction.query_row(
                        "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1 AND password_hash = ?2)",
                        params![account, hash],
                        |row| row.get(0),
                    )?;
                    if !unchanged {
                        return Err(refused(
                            "This link is no longer valid. Request a new email change.",
                        ));
                    }
                    let used: bool = transaction.query_row(
                        "SELECT EXISTS(SELECT 1 FROM accounts WHERE email = ?1)",
                        [&email],
                        |row| row.get(0),
                    )?;
                    if used {
                        return Err(refused("Email: that address is already registered."));
                    }
                    old_email = Some(transaction.query_row(
                        "SELECT email FROM accounts WHERE id = ?1",
                        [&account],
                        |row| row.get(0),
                    )?);
                    transaction.execute(
                        "UPDATE accounts SET email = ?2 WHERE id = ?1",
                        params![account, email],
                    )?;
                    transaction.execute("DELETE FROM account_mail WHERE account_id = ?1 AND purpose IN ('email', 'reset')", [&account])?;
                    None
                }
                "reset" if new_hash.is_some() => {
                    transaction.execute(
                        "UPDATE accounts SET password_hash = ?2 WHERE id = ?1",
                        params![account, new_hash.unwrap()],
                    )?;
                    transaction.execute("DELETE FROM sessions WHERE account_id = ?1", [&account])?;
                    transaction
                        .execute("DELETE FROM account_mail WHERE account_id = ?1", [&account])?;
                    None
                }
                _ => return Err(refused("This link cannot be used for that action.")),
            };
            transaction.execute(
                "DELETE FROM account_mail WHERE token_hash = ?1",
                [hash_token(&token)],
            )?;
            transaction.commit()?;
            if purpose != "register" {
                store.sessions_changed();
            }
            if purpose == "reset" {
                store.passwords.forget_email(&email);
            }
            if purpose != "email" {
                store.passwords.remember(&email, &address, Instant::now());
            }
            Ok((result, code, old_email))
        })
        .await
    }
}

/// A mail link about to be made.
struct Pending<'a> {
    purpose: &'a str,
    account: &'a str,
    email: &'a str,
    name: &'a str,
    password_hash: &'a str,
    code: &'a str,
    invite: &'a str,
    address: &'a str,
}

fn too_many() -> ServerError {
    refused("Too many email requests. Try again after the existing links expire.")
}

fn count(database: &Connection, query: &str, value: &str) -> Result<i64, ServerError> {
    Ok(database.query_row(query, [value], |row| row.get(0))?)
}

/// The mail limits that don't depend on the email: the address's share and,
/// for the purposes that hold one, the global cap. Also forgets expired links.
fn may_send_from(database: &Connection, purpose: &str, address: &str) -> Result<(), ServerError> {
    database.execute("DELETE FROM account_mail WHERE expires <= ?1", [now()])?;
    let from_address = count(
        database,
        "SELECT count(*) FROM account_mail WHERE address = ?1",
        address,
    )?;
    if from_address >= MAX_MAIL_LINKS_PER_ADDRESS {
        return Err(too_many());
    }
    let capped = match purpose {
        "register" | "email" => {
            database.query_row(
                "SELECT count(*) FROM account_mail WHERE purpose IN ('register', 'email')",
                [],
                |row| row.get::<_, i64>(0),
            )? >= MAX_MAIL_LINKS
        }
        UNSENT => {
            count(
                database,
                "SELECT count(*) FROM account_mail WHERE purpose = ?1",
                UNSENT,
            )? >= MAX_UNSENT_RESETS
        }
        _ => false,
    };
    if capped {
        return Err(too_many());
    }
    Ok(())
}

fn may_send(
    database: &Connection,
    purpose: &str,
    email: &str,
    address: &str,
) -> Result<(), ServerError> {
    may_send_from(database, purpose, address)?;
    let recent: i64 = database.query_row(
        "SELECT count(*) FROM account_mail WHERE email = ?1 AND purpose = ?2",
        params![email, purpose],
        |row| row.get(0),
    )?;
    if recent >= MAX_MAIL_LINKS_PER_EMAIL {
        return Err(too_many());
    }
    Ok(())
}

fn unregistered(database: &Connection, email: &str) -> Result<(), ServerError> {
    let used: bool = database.query_row(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE email = ?1)",
        [email],
        |row| row.get(0),
    )?;
    if used {
        return Err(refused("Email: that address is already registered."));
    }
    Ok(())
}

fn may_send_with(database: &Connection, invite: &str) -> Result<(), ServerError> {
    let waiting = count(
        database,
        "SELECT count(*) FROM account_mail WHERE purpose = 'register' AND invite_hash = ?1",
        invite,
    )?;
    if waiting >= MAX_MAIL_LINKS_PER_INVITE {
        return Err(refused(
            "Invite code: too many registrations with this code are waiting on their email. Try again after they expire.",
        ));
    }
    Ok(())
}

fn mail_link(database: &Connection, pending: Pending) -> Result<MailLink, ServerError> {
    may_send(database, pending.purpose, pending.email, pending.address)?;
    if pending.purpose == "register" {
        may_send_with(database, pending.invite)?;
    }
    if matches!(pending.purpose, "register" | "email") {
        unregistered(database, pending.email)?;
    }
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let lifetime = if pending.purpose == "register" || pending.purpose == "email" {
        REGISTRATION_LINK_LIFETIME
    } else {
        RESET_LINK_LIFETIME
    };
    database.execute(
        "INSERT INTO account_mail (token_hash, purpose, account_id, email, display_name, password_hash, device_code, expires, address, invite_hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            hash_token(&token),
            pending.purpose,
            pending.account,
            pending.email,
            pending.name,
            pending.password_hash,
            pending.code,
            now() + lifetime,
            pending.address,
            pending.invite
        ],
    )?;
    Ok(MailLink {
        email: pending.email.into(),
        purpose: pending.purpose.into(),
        token,
    })
}
