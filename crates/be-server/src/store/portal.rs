use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

/// Email-change links outstanding at once. Only a signed-in account with a fresh proof can
/// take one, so anonymous traffic can't fill it.
pub const MAX_MAIL_LINKS: i64 = 1024;
/// Email-change links one address (an IPv6 /64) may have outstanding.
pub const MAX_MAIL_LINKS_PER_ADDRESS: i64 = 8;
/// Email-change links one new email may have outstanding.
pub const MAX_MAIL_LINKS_PER_EMAIL: i64 = 3;
const EMAIL_LINK_LIFETIME: i64 = 3600;
/// Why an email change's link opened in a browser not signed in as the account that asked for
/// it confirms nothing.
pub(crate) const ASKED_ELSEWHERE: &str = "Sign in as the account that asked for this change, or open the link in the browser where you asked for it, then open the link again.";

pub struct AccountSession {
    pub id: String,
    pub kind: String,
    pub created: i64,
    pub current: bool,
}

/// A link to verify a new email, to mail to it.
pub struct MailLink {
    pub email: String,
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
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let valid: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE account_id = ?1 AND token_hash = ?2 AND kind = 'browser' AND expires > ?3)", params![account.to_string(), hash_token(TokenKind::Session, &issuer), now()], |row| row.get(0))?;
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

    /// A browser session for the account with no sign-in, for tests and benches that start
    /// signed in, as `register` makes an account with no code. It counts as a fresh proof, as
    /// signing in does. The portal never calls it: a browser signs in by a code or an
    /// operator's login link.
    pub async fn browser_session(&self, account: Uuid) -> Result<String, ServerError> {
        self.with_database(move |_, database| {
            let token = issue_token(&database, account)?;
            database.execute(
                "UPDATE sessions SET kind = 'browser', expires = ?2, proved = ?3 WHERE token_hash = ?1",
                params![hash_token(TokenKind::Session, &token), now() + 30 * 86400, now()],
            )?;
            Ok(token)
        })
        .await
    }

    pub async fn browser_profile(&self, token: &str) -> Result<Profile, ServerError> {
        let token = token.to_owned();
        self.with_database(move |_, database| {
            let row: Option<(String, String, String)> = database.query_row(
                "SELECT a.id, COALESCE(a.email, ''), a.display_name FROM sessions s JOIN accounts a ON a.id = s.account_id WHERE s.token_hash = ?1 AND s.kind = 'browser' AND s.expires > ?2",
                params![hash_token(TokenKind::Session, &token), now()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).optional()?;
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
            let mut query = database.prepare("SELECT public_id, kind, created, token_hash FROM sessions WHERE account_id = ?1 AND kind IN ('app', 'browser') AND expires > ?2 ORDER BY created DESC, public_id")?;
            let sessions = query
                .query_map(params![account.to_string(), now()], |row| {
                    Ok(AccountSession {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        created: row.get(2)?,
                        current: row.get::<_, String>(3)? == hash_token(TokenKind::Session, &current),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(sessions)
        })
        .await
    }

    /// Ends one of the account's sessions. Ending any but `browser`'s own needs a fresh proof
    /// from it.
    pub async fn revoke_session(
        &self,
        account: Uuid,
        id: &str,
        browser: &str,
    ) -> Result<(), ServerError> {
        let own = {
            let (id, browser) = (id.to_owned(), hash_token(TokenKind::Session, browser));
            self.with_database(move |_, database| {
                Ok(database.query_row(
                    "SELECT EXISTS(SELECT 1 FROM sessions WHERE account_id = ?1 AND public_id = ?2 AND token_hash = ?3)",
                    params![account.to_string(), id, browser],
                    |row| row.get::<_, bool>(0),
                )?)
            })
            .await?
        };
        if !own {
            self.require_fresh(browser).await?;
        }
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

    /// Ends every session of the account but `current`, after a fresh proof from it.
    pub async fn revoke_other_sessions(
        &self,
        account: Uuid,
        current: &str,
    ) -> Result<(), ServerError> {
        self.require_fresh(current).await?;
        let current = hash_token(TokenKind::Session, current);
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

    /// A link to verify a new email for the account, after a fresh proof
    /// (`codes::FRESH_PROOF`) from the browser asking.
    pub async fn email_change_link(
        &self,
        account: Uuid,
        email: &str,
        browser: &str,
        address: &str,
    ) -> Result<MailLink, ServerError> {
        let email = normalize_email(email)?;
        let address = crate::address::address_key_text(address);
        self.require_fresh(browser).await?;
        self.with_database(move |_, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let link = mail_link(&transaction, &account.to_string(), &email, &address)?;
            transaction.commit()?;
            Ok(link)
        })
        .await
    }

    /// Confirms an email change by its link's token, in a browser signed in as `account`, the
    /// account that asked for it: the link alone would let an account that asked for a
    /// stranger's address have the stranger's click attach it. Opened by another account it
    /// confirms nothing and stays unspent. Answers the account's old email (empty for none) and
    /// its new one.
    pub async fn confirm_mail(
        &self,
        token: &str,
        account: Uuid,
    ) -> Result<(String, String), ServerError> {
        let token = token.to_owned();
        self.with_database(move |store, database| {
            let transaction = be_block_sqlite_schema::write_transaction(database)?;
            let row: Option<(String, String)> = transaction
                .query_row(
                    "SELECT account_id, email FROM account_mail WHERE token_hash = ?1 AND expires > ?2",
                    params![hash_token(TokenKind::MailLink, &token), now()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let (asked_by, email) = row.ok_or_else(|| {
                refused("This link is invalid, expired, or already used. Request a new link.")
            })?;
            let account = account.to_string();
            if asked_by != account {
                return Err(refused(ASKED_ELSEWHERE));
            }
            super::codes::unregistered(&transaction, &email)?;
            let old_email: String = transaction.query_row(
                "SELECT COALESCE(email, '') FROM accounts WHERE id = ?1",
                [&account],
                |row| row.get(0),
            )?;
            transaction.execute(
                "UPDATE accounts SET email = ?2 WHERE id = ?1",
                params![account, email],
            )?;
            let moved = transaction.execute(
                "UPDATE identities SET subject = ?3 WHERE account_id = ?1 AND kind = 'email' AND subject = ?2",
                params![account, old_email, email],
            )?;
            // An account made through a provider may have had no email to sign in with.
            if moved == 0 {
                super::codes::add_email_identity(&transaction, &account, &email)?;
            }
            transaction.execute("DELETE FROM account_mail WHERE account_id = ?1", [&account])?;
            transaction.commit()?;
            store.sessions_changed();
            Ok((old_email, email))
        })
        .await
    }
}

fn too_many() -> ServerError {
    refused("Too many email requests. Try again after the existing links expire.")
}

/// The limits on email-change links, which only a signed-in account with a fresh proof asks
/// for: the address's share, the email's, and the global cap. Also forgets expired links.
fn may_send(database: &Connection, email: &str, address: &str) -> Result<(), ServerError> {
    database.execute("DELETE FROM account_mail WHERE expires <= ?1", [now()])?;
    let count = |query: &str, value: &str| -> Result<i64, ServerError> {
        Ok(database.query_row(query, [value], |row| row.get(0))?)
    };
    if count(
        "SELECT count(*) FROM account_mail WHERE address = ?1",
        address,
    )? >= MAX_MAIL_LINKS_PER_ADDRESS
        || count("SELECT count(*) FROM account_mail WHERE email = ?1", email)?
            >= MAX_MAIL_LINKS_PER_EMAIL
        || database.query_row("SELECT count(*) FROM account_mail", [], |row| {
            row.get::<_, i64>(0)
        })? >= MAX_MAIL_LINKS
    {
        return Err(too_many());
    }
    Ok(())
}

fn mail_link(
    database: &Connection,
    account: &str,
    email: &str,
    address: &str,
) -> Result<MailLink, ServerError> {
    may_send(database, email, address)?;
    super::codes::unregistered(database, email)?;
    let token = crate::new_secret();
    database.execute(
        "INSERT INTO account_mail (token_hash, account_id, email, expires, address) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![hash_token(TokenKind::MailLink, &token), account, email, now() + EMAIL_LINK_LIFETIME, address],
    )?;
    Ok(MailLink {
        email: email.into(),
        token,
    })
}
