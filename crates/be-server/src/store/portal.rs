use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

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
        let database = self.database.lock().await;
        let transaction = database.unchecked_transaction()?;
        let valid: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE account_id = ?1 AND token_hash = ?2 AND kind = 'browser' AND expires > ?3)", params![account.to_string(), hash_token(issuer), now()], |row| row.get(0))?;
        if !valid {
            return Err(refused(
                "The approving browser session expired or was revoked. Start sign-in again.",
            ));
        }
        let token = issue_token(&transaction, account)?;
        transaction.commit()?;
        Ok(token)
    }

    pub async fn browser_session(&self, account: Uuid) -> Result<String, ServerError> {
        let database = self.database.lock().await;
        let token = issue_token(&database, account)?;
        database.execute(
            "UPDATE sessions SET kind = 'browser', expires = ?2 WHERE token_hash = ?1",
            params![hash_token(&token), now() + 30 * 86400],
        )?;
        Ok(token)
    }

    pub async fn browser_profile(&self, token: &str) -> Result<Profile, ServerError> {
        let database = self.database.lock().await;
        let row: Option<(String, String, String)> = database.query_row(
            "SELECT a.id, a.email, a.display_name FROM sessions s JOIN accounts a ON a.id = s.account_id WHERE s.token_hash = ?1 AND s.kind = 'browser' AND s.expires > ?2",
            params![hash_token(token), now()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).optional()?;
        let (account, email, display_name) = row.ok_or_else(|| refused("Sign in to continue."))?;
        Ok(Profile {
            account: parse_uuid(&account)?,
            email,
            display_name,
        })
    }

    pub async fn account_sessions(
        &self,
        account: Uuid,
        current: &str,
    ) -> Result<Vec<AccountSession>, ServerError> {
        let database = self.database.lock().await;
        let mut query = database.prepare("SELECT public_id, kind, created, token_hash FROM sessions WHERE account_id = ?1 AND (kind = 'app' OR (kind = 'browser' AND expires > ?2)) ORDER BY created DESC, public_id")?;
        let sessions = query
            .query_map(params![account.to_string(), now()], |row| {
                Ok(AccountSession {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    created: row.get(2)?,
                    current: row.get::<_, String>(3)? == hash_token(current),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(sessions)
    }

    pub async fn revoke_session(&self, account: Uuid, id: &str) -> Result<(), ServerError> {
        self.database.lock().await.execute(
            "DELETE FROM sessions WHERE account_id = ?1 AND public_id = ?2",
            params![account.to_string(), id],
        )?;
        self.sessions_changed();
        Ok(())
    }

    pub async fn revoke_other_sessions(
        &self,
        account: Uuid,
        current: &str,
    ) -> Result<(), ServerError> {
        self.database.lock().await.execute(
            "DELETE FROM sessions WHERE account_id = ?1 AND token_hash != ?2",
            params![account.to_string(), hash_token(current)],
        )?;
        self.sessions_changed();
        Ok(())
    }

    pub async fn update_display_name(&self, account: Uuid, name: &str) -> Result<(), ServerError> {
        validate_name(name)?;
        self.database.lock().await.execute(
            "UPDATE accounts SET display_name = ?2 WHERE id = ?1",
            params![account.to_string(), name.trim()],
        )?;
        Ok(())
    }

    pub async fn check_password(
        &self,
        account: Uuid,
        password: &str,
    ) -> Result<String, ServerError> {
        let profile = self.profile(account).await?;
        if let Some(wait) = self.locked_out(&profile.email) {
            return Err(refused(&format!(
                "Too many failed attempts. Try again in {} seconds.",
                wait.as_secs().max(1)
            )));
        }
        let hash: String = self.database.lock().await.query_row(
            "SELECT password_hash FROM accounts WHERE id = ?1 AND local_profile = 0",
            [account.to_string()],
            |row| row.get(0),
        )?;
        if !self.verify(password.into(), hash.clone()).await? {
            self.failed(&profile.email);
            return Err(refused("Current password: incorrect password."));
        }
        self.failures.lock().unwrap().remove(&profile.email);
        Ok(hash)
    }

    pub async fn change_password(
        &self,
        account: Uuid,
        current: &str,
        password: &str,
        browser_token: &str,
    ) -> Result<String, ServerError> {
        let previous_hash = self.check_password(account, current).await?;
        validate_password(password)?;
        let hash = self.hash(password.into()).await?;
        let database = self.database.lock().await;
        let transaction = database.unchecked_transaction()?;
        let valid: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE account_id = ?1 AND token_hash = ?2 AND kind = 'browser' AND expires > ?3 AND EXISTS(SELECT 1 FROM accounts WHERE id = ?1 AND password_hash = ?4))", params![account.to_string(), hash_token(browser_token), now(), previous_hash], |row| row.get(0))?;
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
        self.sessions_changed();
        Ok(token)
    }

    pub async fn registration_link(
        &self,
        email: &str,
        name: &str,
        password: &str,
        code: &str,
    ) -> Result<MailLink, ServerError> {
        let email = normalize_email(email)?;
        validate_name(name)?;
        validate_password(password)?;
        let hash = self.hash(password.into()).await?;
        self.mail_link("register", "", &email, name.trim(), &hash, code)
            .await
    }

    pub async fn email_change_link(
        &self,
        account: Uuid,
        email: &str,
        password: &str,
    ) -> Result<MailLink, ServerError> {
        let previous_hash = self.check_password(account, password).await?;
        let email = normalize_email(email)?;
        self.mail_link(
            "email",
            &account.to_string(),
            &email,
            "",
            &previous_hash,
            "",
        )
        .await
    }

    pub async fn password_reset_link(
        &self,
        email: &str,
        code: &str,
    ) -> Result<Option<MailLink>, ServerError> {
        let Ok(email) = normalize_email(email) else {
            return Ok(None);
        };
        let account: Option<String> = self
            .database
            .lock()
            .await
            .query_row(
                "SELECT id FROM accounts WHERE email = ?1 AND local_profile = 0",
                [&email],
                |row| row.get(0),
            )
            .optional()?;
        let Some(account) = account else {
            return Ok(None);
        };
        match self
            .mail_link("reset", &account, &email, "", "", code)
            .await
        {
            Ok(link) => Ok(Some(link)),
            Err(ServerError::Refused(..)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn mail_link(
        &self,
        purpose: &str,
        account: &str,
        email: &str,
        name: &str,
        password_hash: &str,
        code: &str,
    ) -> Result<MailLink, ServerError> {
        let database = self.database.lock().await;
        let transaction = database.unchecked_transaction()?;
        transaction.execute("DELETE FROM account_mail WHERE expires <= ?1", [now()])?;
        let count: i64 =
            transaction.query_row("SELECT count(*) FROM account_mail", [], |row| row.get(0))?;
        let recent: i64 = transaction.query_row(
            "SELECT count(*) FROM account_mail WHERE email = ?1 AND purpose = ?2",
            params![email, purpose],
            |row| row.get(0),
        )?;
        if count >= 1024 || recent >= 3 {
            return Err(refused(
                "Too many email requests. Try again after the existing links expire.",
            ));
        }
        if purpose != "reset" {
            let used: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM accounts WHERE email = ?1)",
                [email],
                |row| row.get(0),
            )?;
            if used {
                return Err(refused("Email: that address is already registered."));
            }
        }
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        transaction.execute("INSERT INTO account_mail (token_hash, purpose, account_id, email, display_name, password_hash, device_code, expires) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", params![hash_token(&token), purpose, account, email, name, password_hash, code, now() + if purpose == "reset" { 1800 } else { 3600 }])?;
        transaction.commit()?;
        Ok(MailLink {
            email: email.into(),
            purpose: purpose.into(),
            token,
        })
    }

    pub async fn mail_purpose(&self, token: &str) -> Result<String, ServerError> {
        self.database
            .lock()
            .await
            .query_row(
                "SELECT purpose FROM account_mail WHERE token_hash = ?1 AND expires > ?2",
                params![hash_token(token), now()],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| refused("This link is invalid, expired, or already used."))
    }

    pub async fn confirm_mail(
        &self,
        token: &str,
        password: Option<&str>,
    ) -> Result<(Option<Uuid>, String, Option<String>), ServerError> {
        let new_hash = match password {
            Some(password) => {
                validate_password(password)?;
                Some(self.hash(password.into()).await?)
            }
            None => None,
        };
        let database = self.database.lock().await;
        let transaction = database.unchecked_transaction()?;
        let row: Option<(String, String, String, String, String, String)> = transaction.query_row("SELECT purpose, account_id, email, display_name, password_hash, device_code FROM account_mail WHERE token_hash = ?1 AND expires > ?2", params![hash_token(token), now()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))).optional()?;
        let (purpose, account, email, name, hash, code) = row.ok_or_else(|| {
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
            [hash_token(token)],
        )?;
        transaction.commit()?;
        self.sessions_changed();
        if purpose == "reset" {
            self.failures.lock().unwrap().remove(&email);
        }
        Ok((result, code, old_email))
    }
}
