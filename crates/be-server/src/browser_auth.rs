use std::{
    collections::{BTreeSet, HashMap},
    net::IpAddr,
    sync::Mutex,
    time::Duration,
};

use be_block_protocol::ErrorCode;
use tokio::time::Instant;
use uuid::Uuid;

use crate::{ServerError, Services, address::address_key};

/// How long a pending sign-in or a form token lasts.
pub(crate) const LIFETIME: Duration = Duration::from_secs(600);
/// Pending sign-ins, and open form tokens, the whole server holds at once. Making one more
/// lets go of the oldest, so filling it locks no one out.
pub(crate) const LIMIT: usize = 65536;
/// Pending sign-ins one address (see [`address_key`]) holds; beginning another lets go of its
/// oldest.
pub(crate) const PENDING_PER_ADDRESS: usize = 8;
/// Form tokens one address holds; issuing another lets go of its oldest.
const FORMS_PER_ADDRESS: usize = 64;
/// The fewest seconds a client waits between two polls of a sign-in, at first.
pub(crate) const POLL_INTERVAL: Duration = Duration::from_secs(3);
/// What polling too soon adds to a sign-in's interval (RFC 8628).
pub(crate) const SLOW_DOWN: Duration = Duration::from_secs(5);

pub(crate) struct Sessions {
    pending: Mutex<Bounded<Pending>>,
    forms: Mutex<Bounded<Form>>,
    /// A new sign-in's interval (`ServerConfig::sign_in_poll_interval`).
    interval: Duration,
}

impl Default for Sessions {
    fn default() -> Self {
        Self::polled_every(POLL_INTERVAL)
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

    #[cfg(test)]
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
    secret: String,
    denied: bool,
    expires: Instant,
    account: Option<Uuid>,
    approved_by: String,
    /// The fewest seconds between two polls; polling sooner adds `SLOW_DOWN` to it.
    interval: Duration,
    polled: Option<Instant>,
}

/// Where a pending sign-in stands, as a poll finds it.
pub(crate) enum Polled {
    Pending,
    /// Approved by `issuer`, the browser session that approved it; the sign-in is gone.
    Approved {
        account: Uuid,
        issuer: String,
    },
    Denied,
    Expired,
}

fn invalid(message: &str) -> ServerError {
    ServerError::Refused(ErrorCode::InvalidRequest, message.into())
}

impl Sessions {
    pub(crate) fn polled_every(interval: Duration) -> Self {
        Self {
            pending: Mutex::new(Bounded::new(LIMIT, PENDING_PER_ADDRESS)),
            forms: Mutex::new(Bounded::new(LIMIT, FORMS_PER_ADDRESS)),
            interval,
        }
    }

    /// Begins a browser sign-in from `address`: its code, its secret, the fewest seconds between
    /// polls and the seconds it lasts.
    pub(crate) fn begin(&self, address: IpAddr) -> (String, String, Duration, Duration) {
        let random = Uuid::new_v4().simple().to_string();
        let code = format!("{}-{}-{}", &random[..4], &random[4..8], &random[8..12]);
        let secret = Uuid::new_v4().to_string();
        self.pending.lock().unwrap().insert(
            code.clone(),
            Pending {
                secret: secret.clone(),
                denied: false,
                expires: Instant::now() + LIFETIME,
                account: None,
                approved_by: String::new(),
                interval: self.interval,
                polled: None,
            },
            address,
        );
        (code, secret, self.interval, LIFETIME)
    }

    /// Where the sign-in begun with `code` and `secret` stands. A poll sooner than its interval
    /// is refused with `SlowDown`, and the interval grows by `SLOW_DOWN`. An approved or denied
    /// sign-in is answered once and let go of; a wrong secret reads as expired.
    pub(crate) fn poll(&self, code: &str, secret: &str) -> Result<Polled, ServerError> {
        let mut pending = self.pending.lock().unwrap();
        let now = Instant::now();
        let Some(entry) = pending
            .get_mut(code)
            .filter(|entry| entry.secret == secret)
        else {
            return Ok(Polled::Expired);
        };
        if entry.expires <= now {
            pending.remove(code);
            return Ok(Polled::Expired);
        }
        if let Some(polled) = entry.polled
            && polled + entry.interval > now
        {
            entry.interval += SLOW_DOWN;
            return Err(ServerError::Refused(
                ErrorCode::SlowDown,
                format!(
                    "poll this sign-in at most every {} seconds",
                    entry.interval.as_secs()
                ),
            ));
        }
        entry.polled = Some(now);
        if entry.denied {
            pending.remove(code);
            return Ok(Polled::Denied);
        }
        let Some(account) = entry.account else {
            return Ok(Polled::Pending);
        };
        let issuer = entry.approved_by.clone();
        pending.remove(code);
        Ok(Polled::Approved { account, issuer })
    }

    pub(crate) fn form(&self, owner: &str, address: IpAddr) -> String {
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
        token
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
