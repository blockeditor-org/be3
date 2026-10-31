use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use be_protocol::ErrorCode;
use tokio::time::Instant;
use uuid::Uuid;

use crate::{ServerError, Services};

const LIFETIME: Duration = Duration::from_secs(600);
const LIMIT: usize = 1024;

#[derive(Default)]
pub(crate) struct Sessions {
    pending: Mutex<HashMap<String, Pending>>,
    forms: Mutex<HashMap<String, (Instant, String)>>,
}

struct Pending {
    client: u64,
    secret: String,
    denied: bool,
    expires: Instant,
    account: Option<Uuid>,
    approved_by: String,
    decided: tokio::sync::watch::Sender<()>,
    _cancel: tokio::sync::oneshot::Sender<()>,
}

pub(crate) enum Finished {
    Approved { account: Uuid, issuer: String },
    Waiting(tokio::sync::watch::Receiver<()>),
}

fn invalid(message: &str) -> ServerError {
    ServerError::Refused(ErrorCode::InvalidRequest, message.into())
}

impl Sessions {
    pub(crate) fn begin(self: &Arc<Self>, client: u64) -> Result<(String, String), ServerError> {
        let mut pending = self.pending.lock().unwrap();
        pending.retain(|_, entry| entry.client != client && entry.expires > Instant::now());
        if pending.len() >= LIMIT {
            return Err(invalid("too many pending sign-ins; try again later"));
        }
        let random = Uuid::new_v4().simple().to_string();
        let code = format!("{}-{}-{}", &random[..4], &random[4..8], &random[8..12]);
        let secret = Uuid::new_v4().to_string();
        let (cancel, cancelled) = tokio::sync::oneshot::channel();
        pending.insert(
            code.clone(),
            Pending {
                client,
                secret: secret.clone(),
                denied: false,
                expires: Instant::now() + LIFETIME,
                account: None,
                approved_by: String::new(),
                decided: tokio::sync::watch::Sender::new(()),
                _cancel: cancel,
            },
        );
        let sessions = Arc::downgrade(self);
        let expiring = code.clone();
        tokio::spawn(async move {
            tokio::select! { _ = tokio::time::sleep(LIFETIME) => {}, _ = cancelled => return }
            if let Some(sessions) = sessions.upgrade() {
                sessions.pending.lock().unwrap().remove(&expiring);
            }
        });
        Ok((code, secret))
    }

    pub(crate) fn finish(
        &self,
        client: u64,
        code: &str,
        secret: &str,
    ) -> Result<Finished, ServerError> {
        let mut pending = self.pending.lock().unwrap();
        let entry = pending
            .get_mut(code)
            .filter(|entry| entry.secret == secret && entry.expires > Instant::now())
            .ok_or_else(|| invalid("this sign-in expired or was cancelled"))?;
        if entry.denied {
            return Err(invalid("Sign-in was denied in the browser."));
        }
        entry.client = client;
        let Some(account) = entry.account else {
            return Ok(Finished::Waiting(entry.decided.subscribe()));
        };
        let issuer = entry.approved_by.clone();
        pending.remove(code);
        Ok(Finished::Approved { account, issuer })
    }

    pub(crate) fn form(&self, owner: &str) -> Result<String, ServerError> {
        let mut forms = self.forms.lock().unwrap();
        forms.retain(|_, (expires, _)| *expires > Instant::now());
        if forms.len() >= LIMIT {
            return Err(invalid("too many open sign-in pages; try again later"));
        }
        let token = Uuid::new_v4().to_string();
        forms.insert(token.clone(), (Instant::now() + LIFETIME, owner.into()));
        Ok(token)
    }

    pub(crate) fn consume_form(&self, token: &str, owner: &str) -> Result<(), ServerError> {
        self.forms
            .lock()
            .unwrap()
            .remove(token)
            .filter(|(expires, bound)| *expires > Instant::now() && bound == owner)
            .ok_or_else(|| invalid("this page expired; reload it and try again"))?;
        Ok(())
    }
}

pub(crate) fn fields(body: &str) -> HashMap<String, String> {
    body.split('&')
        .filter_map(|part| {
            let (key, value) = part.split_once('=')?;
            Some((decode(key)?, decode(value)?))
        })
        .collect()
}

fn decode(value: &str) -> Option<String> {
    let mut bytes = Vec::new();
    let mut source = value.bytes();
    while let Some(byte) = source.next() {
        bytes.push(match byte {
            b'+' => b' ',
            b'%' => u8::from_str_radix(
                &String::from_utf8(vec![source.next()?, source.next()?]).ok()?,
                16,
            )
            .ok()?,
            other => other,
        });
    }
    String::from_utf8(bytes).ok()
}

pub(crate) async fn decide(
    services: &Services,
    code: &str,
    account: Option<Uuid>,
    issuer: &str,
) -> Result<(), ServerError> {
    if !services.config.allow_login {
        return Err(invalid("This server is not accepting sign-ins."));
    }
    let code = code.trim().to_ascii_lowercase();
    let mut pending = services.browser_auth.pending.lock().unwrap();
    let entry = pending.get_mut(&code).filter(|entry| entry.expires > Instant::now() && entry.account.is_none() && !entry.denied)
        .ok_or_else(|| invalid("This code is invalid, expired, denied, or already approved. Start sign-in again in Block."))?;
    entry.account = account;
    entry.approved_by = issuer.into();
    entry.denied = account.is_none();
    entry.decided.send_replace(());
    Ok(())
}

pub(crate) fn code_status(services: &Services, code: &str) -> Result<(), ServerError> {
    let pending = services.browser_auth.pending.lock().unwrap();
    match pending.get(&code.trim().to_ascii_lowercase()) {
        None => Err(invalid(
            "This code is invalid or expired. Start sign-in again in Block.",
        )),
        Some(entry) if entry.expires <= Instant::now() => Err(invalid(
            "This sign-in expired. Start sign-in again in Block.",
        )),
        Some(entry) if entry.denied => Err(invalid(
            "This sign-in was denied. Start sign-in again in Block.",
        )),
        Some(entry) if entry.account.is_some() => Err(invalid(
            "This sign-in was already approved. Return to Block.",
        )),
        Some(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests;
