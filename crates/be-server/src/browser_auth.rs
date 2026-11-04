use std::{
    collections::{BTreeSet, HashMap},
    net::IpAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use be_block_protocol::ErrorCode;
use tokio::time::Instant;
use uuid::Uuid;

use crate::{ServerError, Services, address::address_key};

/// How long a pending sign-in or a form token lasts, and so the longest a connection that is not
/// signed in stays open while it waits on one.
pub(crate) const LIFETIME: Duration = Duration::from_secs(600);
/// Pending sign-ins, and open form tokens, the whole server holds at once. Making one more
/// lets go of the oldest, so filling it locks no one out.
pub(crate) const LIMIT: usize = 65536;
/// Pending sign-ins one address (see [`address_key`]) holds; beginning another lets go of its
/// oldest.
pub(crate) const PENDING_PER_ADDRESS: usize = 8;
/// Form tokens one address holds; issuing another lets go of its oldest.
const FORMS_PER_ADDRESS: usize = 64;

pub(crate) struct Sessions {
    pending: Mutex<Bounded<Pending>>,
    forms: Mutex<Bounded<Form>>,
}

impl Default for Sessions {
    fn default() -> Self {
        Self {
            pending: Mutex::new(Bounded::new(LIMIT, PENDING_PER_ADDRESS)),
            forms: Mutex::new(Bounded::new(LIMIT, FORMS_PER_ADDRESS)),
        }
    }
}

/// Entries by id, each from an address and lasting `LIFETIME` from when it was made, indexed by
/// age overall and per address, so making room, expiring and evicting take a few steps each
/// rather than a pass over every entry.
pub(crate) struct Bounded<V> {
    entries: HashMap<String, Held<V>>,
    by_age: BTreeSet<(Instant, u64, String)>,
    by_address: HashMap<IpAddr, BTreeSet<(Instant, u64, String)>>,
    next: u64,
    limit: usize,
    per_address: usize,
}

struct Held<V> {
    value: V,
    key: IpAddr,
    made: (Instant, u64),
}

impl<V> Bounded<V> {
    pub(crate) fn new(limit: usize, per_address: usize) -> Self {
        Self {
            entries: HashMap::new(),
            by_age: BTreeSet::new(),
            by_address: HashMap::new(),
            next: 0,
            limit,
            per_address,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Adds an entry from `address`, first letting go of what has expired, of the address's
    /// oldest while it holds `per_address`, and of the oldest of all while there are `limit`.
    pub(crate) fn insert(&mut self, id: String, value: V, address: IpAddr) {
        let now = Instant::now();
        while let Some((made, _, oldest)) = self.by_age.first().cloned()
            && made + LIFETIME <= now
        {
            self.remove(&oldest);
        }
        let key = address_key(address);
        while self
            .by_address
            .get(&key)
            .is_some_and(|held| held.len() >= self.per_address)
        {
            let Some((_, _, oldest)) = self.by_address[&key].first().cloned() else {
                break;
            };
            self.remove(&oldest);
        }
        while self.entries.len() >= self.limit {
            let Some((_, _, oldest)) = self.by_age.first().cloned() else {
                break;
            };
            self.remove(&oldest);
        }
        self.remove(&id);
        let made = (now, self.next);
        self.next += 1;
        self.by_age.insert((made.0, made.1, id.clone()));
        self.by_address
            .entry(key)
            .or_default()
            .insert((made.0, made.1, id.clone()));
        self.entries.insert(id, Held { value, key, made });
    }

    pub(crate) fn get(&self, id: &str) -> Option<&V> {
        self.entries.get(id).map(|held| &held.value)
    }

    pub(crate) fn get_mut(&mut self, id: &str) -> Option<&mut V> {
        self.entries.get_mut(id).map(|held| &mut held.value)
    }

    pub(crate) fn remove(&mut self, id: &str) -> Option<V> {
        let held = self.entries.remove(id)?;
        let entry = (held.made.0, held.made.1, id.to_owned());
        self.by_age.remove(&entry);
        if let Some(of) = self.by_address.get_mut(&held.key) {
            of.remove(&entry);
            if of.is_empty() {
                self.by_address.remove(&held.key);
            }
        }
        Some(held.value)
    }
}

struct Form {
    expires: Instant,
    owner: String,
}

struct Pending {
    client: u64,
    secret: String,
    denied: bool,
    expires: Instant,
    account: Option<Uuid>,
    approved_by: String,
    decided: tokio::sync::watch::Sender<()>,
    /// The one call waiting on this sign-in; replacing it tells the older call to stop.
    waiter: Option<tokio::sync::oneshot::Sender<()>>,
    /// Held for the connection whose sign-in this is; dropped, and so telling it, once the
    /// sign-in ends, expires, is let go of, or another connection takes it over.
    holder: tokio::sync::oneshot::Sender<()>,
    _cancel: tokio::sync::oneshot::Sender<()>,
}

/// A browser sign-in a connection began or waits on, which keeps the connection open while it is
/// pending, at most `LIFETIME` after the connection stopped having an account.
pub(crate) struct SigningIn {
    pub(crate) code: String,
    pub(crate) until: Instant,
    /// Ends once the sign-in is no longer this connection's.
    pub(crate) gone: tokio::sync::oneshot::Receiver<()>,
}

impl SigningIn {
    /// Whether the sign-in is no longer this connection's.
    pub(crate) fn ended(&mut self) -> bool {
        matches!(
            self.gone.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Closed)
        )
    }
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
    /// The sign-in, when this call moved it to its connection from another.
    pub(crate) signing_in: Option<SigningIn>,
}

fn invalid(message: &str) -> ServerError {
    ServerError::Refused(ErrorCode::InvalidRequest, message.into())
}

impl Sessions {
    /// Begins a browser sign-in for connection `client`, letting go of the one it began before,
    /// `previous`, so a connection holds one at a time.
    pub(crate) fn begin(
        self: &Arc<Self>,
        client: u64,
        address: IpAddr,
        previous: Option<&str>,
    ) -> Result<(String, String, SigningIn), ServerError> {
        let mut pending = self.pending.lock().unwrap();
        if let Some(previous) = previous
            && pending
                .get(previous)
                .is_some_and(|entry| entry.client == client)
        {
            pending.remove(previous);
        }
        let random = Uuid::new_v4().simple().to_string();
        let code = format!("{}-{}-{}", &random[..4], &random[4..8], &random[8..12]);
        let secret = Uuid::new_v4().to_string();
        let (cancel, cancelled) = tokio::sync::oneshot::channel();
        let (holder, gone) = tokio::sync::oneshot::channel();
        let expires = Instant::now() + LIFETIME;
        pending.insert(
            code.clone(),
            Pending {
                client,
                secret: secret.clone(),
                denied: false,
                expires,
                account: None,
                approved_by: String::new(),
                decided: tokio::sync::watch::Sender::new(()),
                waiter: None,
                holder,
                _cancel: cancel,
            },
            address,
        );
        let sessions = Arc::downgrade(self);
        let expiring = code.clone();
        tokio::spawn(async move {
            tokio::select! { _ = tokio::time::sleep(LIFETIME) => {}, _ = cancelled => return }
            if let Some(sessions) = sessions.upgrade() {
                sessions.pending.lock().unwrap().remove(&expiring);
            }
        });
        let signing_in = SigningIn {
            code: code.clone(),
            until: expires,
            gone,
        };
        Ok((code, secret, signing_in))
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
        let moved = std::mem::replace(&mut entry.client, client) != client;
        let Some(account) = entry.account else {
            let (waiter, replaced) = tokio::sync::oneshot::channel();
            entry.waiter = Some(waiter);
            let signing_in = moved.then(|| {
                let (holder, gone) = tokio::sync::oneshot::channel();
                entry.holder = holder;
                SigningIn {
                    code: code.to_owned(),
                    until: entry.expires,
                    gone,
                }
            });
            return Ok(Finished::Waiting(Waiting {
                decided: entry.decided.subscribe(),
                replaced,
                signing_in,
            }));
        };
        let issuer = entry.approved_by.clone();
        pending.remove(code);
        Ok(Finished::Approved { account, issuer })
    }

    pub(crate) fn form(&self, owner: &str, address: IpAddr) -> Result<String, ServerError> {
        let mut forms = self.forms.lock().unwrap();
        let token = Uuid::new_v4().to_string();
        forms.insert(
            token.clone(),
            Form {
                expires: Instant::now() + LIFETIME,
                owner: owner.into(),
            },
            address,
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
