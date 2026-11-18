use std::{
    collections::{BTreeSet, HashMap},
    net::IpAddr,
    sync::Mutex,
    time::Duration,
};

use be_block_protocol::ErrorCode;
use tokio::time::Instant;
use uuid::Uuid;

use crate::{
    ServerError, Services,
    address::{address_key, unsigned_key},
};

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
/// Pending sign-ins one network (see [`unsigned_key`]) holds, past which beginning another lets
/// go of the network's oldest. A /48 holds 65536 /64s, each with its own
/// `PENDING_PER_ADDRESS`, so without it one network fills `LIMIT` and pushes out everyone
/// else's. 256 is 32 addresses' whole share, more than a site's people begin within
/// `LIFETIME`, and `LIMIT` takes 256 networks to fill.
pub(crate) const PENDING_PER_NETWORK: usize = 256;
/// Form tokens one network holds, past which issuing another lets go of the network's oldest:
/// 16 addresses' whole share, for a site whose people load the portal's pages at once, and
/// `LIMIT` takes 64 networks to fill.
const FORMS_PER_NETWORK: usize = 1024;
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
/// age overall, per address and per network, so making room, expiring and evicting take a few
/// steps each rather than a pass over every entry.
pub(crate) struct Bounded<V> {
    entries: HashMap<String, Held<V>>,
    by_age: BTreeSet<Age>,
    by_address: Group,
    by_network: Group,
    next: u64,
    limit: usize,
}

type Age = (Instant, u64, String);

/// Entries by what one of the limits counts by, each key holding at most `limit`.
struct Group {
    held: HashMap<IpAddr, BTreeSet<Age>>,
    limit: usize,
    key: fn(IpAddr) -> IpAddr,
}

impl Group {
    fn new(limit: usize, key: fn(IpAddr) -> IpAddr) -> Self {
        Self {
            held: HashMap::new(),
            limit,
            key,
        }
    }

    /// The oldest entry counted with `address`'s while they hold the limit.
    fn over(&self, address: IpAddr) -> Option<String> {
        let held = self.held.get(&(self.key)(address))?;
        (held.len() >= self.limit)
            .then(|| held.first().map(|(_, _, id)| id.clone()))
            .flatten()
    }

    fn insert(&mut self, address: IpAddr, entry: Age) {
        self.held
            .entry((self.key)(address))
            .or_default()
            .insert(entry);
    }

    fn remove(&mut self, address: IpAddr, entry: &Age) {
        let key = (self.key)(address);
        if let Some(of) = self.held.get_mut(&key) {
            of.remove(entry);
            if of.is_empty() {
                self.held.remove(&key);
            }
        }
    }
}

struct Held<V> {
    value: V,
    address: IpAddr,
    made: (Instant, u64),
}

impl<V> Bounded<V> {
    pub(crate) fn new(limit: usize, per_address: usize, per_network: usize) -> Self {
        Self {
            entries: HashMap::new(),
            by_age: BTreeSet::new(),
            by_address: Group::new(per_address, address_key),
            by_network: Group::new(per_network, unsigned_key),
            next: 0,
            limit,
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Adds an entry from `address`, first letting go of what has expired, of the address's
    /// oldest while it holds its limit, of its network's oldest while that holds its own, and of
    /// the oldest of all while there are `limit`.
    pub(crate) fn insert(&mut self, id: String, value: V, address: IpAddr) {
        let now = Instant::now();
        while let Some((made, _, oldest)) = self.by_age.first().cloned()
            && made + LIFETIME <= now
        {
            self.remove(&oldest);
        }
        while let Some(oldest) = self.by_address.over(address) {
            self.remove(&oldest);
        }
        while let Some(oldest) = self.by_network.over(address) {
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
        let entry = (made.0, made.1, id.clone());
        self.by_age.insert(entry.clone());
        self.by_address.insert(address, entry.clone());
        self.by_network.insert(address, entry);
        self.entries.insert(
            id,
            Held {
                value,
                address,
                made,
            },
        );
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
        self.by_address.remove(held.address, &entry);
        self.by_network.remove(held.address, &entry);
        Some(held.value)
    }
}

struct Form {
    expires: Instant,
    owner: String,
    /// The passkey ceremony the page offers, which posting its form spends.
    ceremony: Option<crate::passkeys::Ceremony>,
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
    /// Where the device asking connected from, which the approve page shows.
    address: IpAddr,
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
            pending: Mutex::new(Bounded::new(
                LIMIT,
                PENDING_PER_ADDRESS,
                PENDING_PER_NETWORK,
            )),
            forms: Mutex::new(Bounded::new(LIMIT, FORMS_PER_ADDRESS, FORMS_PER_NETWORK)),
            interval,
        }
    }

    /// Begins a browser sign-in from `address`: its code, its secret, the fewest seconds between
    /// polls and the seconds it lasts.
    pub(crate) fn begin(&self, address: IpAddr) -> (String, String, Duration, Duration) {
        let random = Uuid::new_v4().simple().to_string();
        let code = format!("{}-{}-{}", &random[..4], &random[4..8], &random[8..12]);
        let secret = crate::new_secret();
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
                address,
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
        let Some(entry) = pending.get_mut(code).filter(|entry| entry.secret == secret) else {
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

    /// A page's form token for the session `owner`, holding the passkey ceremony the page
    /// offers, if it offers one.
    pub(crate) fn form(
        &self,
        owner: &str,
        address: IpAddr,
        ceremony: Option<crate::passkeys::Ceremony>,
    ) -> String {
        let mut forms = self.forms.lock().unwrap();
        let token = crate::new_secret();
        forms.insert(
            token.clone(),
            Form {
                expires: Instant::now() + LIFETIME,
                owner: owner.into(),
                ceremony,
            },
            address,
        );
        token
    }

    /// Spends a form token the session `owner` was given, answering the passkey ceremony its
    /// page began, if it began one.
    pub(crate) fn consume_form(
        &self,
        token: &str,
        owner: &str,
    ) -> Result<Option<crate::passkeys::Ceremony>, ServerError> {
        let form = self
            .forms
            .lock()
            .unwrap()
            .remove(token)
            .filter(|form| form.expires > Instant::now() && form.owner == owner)
            .ok_or_else(|| invalid("this page expired; reload it and try again"))?;
        Ok(form.ceremony)
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

/// Whether `code` may still be approved, and if so, where the device asking connected from and
/// how long ago it asked.
pub(crate) fn code_status(
    services: &Services,
    code: &str,
) -> Result<(IpAddr, Duration), ServerError> {
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
        Some(entry) => Ok((
            entry.address,
            LIFETIME.saturating_sub(entry.expires.saturating_duration_since(Instant::now())),
        )),
    }
}

#[cfg(test)]
mod tests;
