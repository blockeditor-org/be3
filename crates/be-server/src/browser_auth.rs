use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use be_block_protocol::ErrorCode;
use tokio::time::Instant;
use uuid::Uuid;

use crate::{ServerError, Services, address::address_key};

const LIFETIME: Duration = Duration::from_secs(600);
/// Pending sign-ins, and open form tokens, the whole server holds at once.
const LIMIT: usize = 65536;
/// Pending sign-ins one address (see [`address_key`]) holds; beginning another lets go of its
/// oldest, so no address can fill `LIMIT` and lock everyone else out.
const PENDING_PER_ADDRESS: usize = 8;
/// Form tokens one address holds; issuing another lets go of its oldest.
const FORMS_PER_ADDRESS: usize = 64;

#[derive(Default)]
pub(crate) struct Sessions {
    pending: Mutex<HashMap<String, Pending>>,
    forms: Mutex<HashMap<String, Form>>,
}

struct Form {
    expires: Instant,
    owner: String,
    address: IpAddr,
}

struct Pending {
    client: u64,
    address: IpAddr,
    secret: String,
    denied: bool,
    expires: Instant,
    account: Option<Uuid>,
    approved_by: String,
    decided: tokio::sync::watch::Sender<()>,
    /// The one call waiting on this sign-in; replacing it tells the older call to stop.
    waiter: Option<tokio::sync::oneshot::Sender<()>>,
    _cancel: tokio::sync::oneshot::Sender<()>,
}

pub(crate) enum Finished {
    Approved { account: Uuid, issuer: String },
    Waiting(Waiting),
}

pub(crate) struct Waiting {
    /// Changes once the browser decides, and ends if the sign-in expires.
    pub(crate) decided: tokio::sync::watch::Receiver<()>,
    /// Ends once a newer call waits on the same sign-in.
    pub(crate) replaced: tokio::sync::oneshot::Receiver<()>,
    pub(crate) expires: Instant,
}

/// Lets go of `address`'s oldest entries until it holds fewer than `limit`.
fn make_room<V>(
    entries: &mut HashMap<String, V>,
    address: IpAddr,
    limit: usize,
    of: impl Fn(&V) -> (IpAddr, Instant),
) {
    let key = address_key(address);
    let mut held: Vec<(Instant, String)> = entries
        .iter()
        .filter(|(_, entry)| address_key(of(entry).0) == key)
        .map(|(id, entry)| (of(entry).1, id.clone()))
        .collect();
    if held.len() < limit {
        return;
    }
    held.sort();
    for (_, id) in &held[..=held.len() - limit] {
        entries.remove(id);
    }
}

fn invalid(message: &str) -> ServerError {
    ServerError::Refused(ErrorCode::InvalidRequest, message.into())
}

impl Sessions {
    pub(crate) fn begin(
        self: &Arc<Self>,
        client: u64,
        address: IpAddr,
    ) -> Result<(String, String, Instant), ServerError> {
        let mut pending = self.pending.lock().unwrap();
        pending.retain(|_, entry| entry.client != client && entry.expires > Instant::now());
        make_room(&mut pending, address, PENDING_PER_ADDRESS, |entry| {
            (entry.address, entry.expires)
        });
        if pending.len() >= LIMIT {
            return Err(invalid("too many pending sign-ins; try again later"));
        }
        let random = Uuid::new_v4().simple().to_string();
        let code = format!("{}-{}-{}", &random[..4], &random[4..8], &random[8..12]);
        let secret = Uuid::new_v4().to_string();
        let (cancel, cancelled) = tokio::sync::oneshot::channel();
        let expires = Instant::now() + LIFETIME;
        pending.insert(
            code.clone(),
            Pending {
                client,
                address,
                secret: secret.clone(),
                denied: false,
                expires,
                account: None,
                approved_by: String::new(),
                decided: tokio::sync::watch::Sender::new(()),
                waiter: None,
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
        Ok((code, secret, expires))
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
            let (waiter, replaced) = tokio::sync::oneshot::channel();
            entry.waiter = Some(waiter);
            return Ok(Finished::Waiting(Waiting {
                decided: entry.decided.subscribe(),
                replaced,
                expires: entry.expires,
            }));
        };
        let issuer = entry.approved_by.clone();
        pending.remove(code);
        Ok(Finished::Approved { account, issuer })
    }

    pub(crate) fn form(&self, owner: &str, address: IpAddr) -> Result<String, ServerError> {
        let mut forms = self.forms.lock().unwrap();
        forms.retain(|_, form| form.expires > Instant::now());
        make_room(&mut forms, address, FORMS_PER_ADDRESS, |form| {
            (form.address, form.expires)
        });
        if forms.len() >= LIMIT {
            return Err(invalid("too many open sign-in pages; try again later"));
        }
        let token = Uuid::new_v4().to_string();
        forms.insert(
            token.clone(),
            Form {
                expires: Instant::now() + LIFETIME,
                owner: owner.into(),
                address,
            },
        );
        Ok(token)
    }

    pub(crate) fn consume_form(&self, token: &str, owner: &str) -> Result<(), ServerError> {
        self.forms
            .lock()
            .unwrap()
            .remove(token)
            .filter(|form| form.expires > Instant::now() && form.owner == owner)
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
        .ok_or_else(|| invalid("This code is invalid, expired, denied, or already approved. Start sign-in again in be-host."))?;
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
            "This code is invalid or expired. Start sign-in again in be-host.",
        )),
        Some(entry) if entry.expires <= Instant::now() => Err(invalid(
            "This sign-in expired. Start sign-in again in be-host.",
        )),
        Some(entry) if entry.denied => Err(invalid(
            "This sign-in was denied. Start sign-in again in be-host.",
        )),
        Some(entry) if entry.account.is_some() => Err(invalid(
            "This sign-in was already approved. Return to be-host.",
        )),
        Some(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests;
