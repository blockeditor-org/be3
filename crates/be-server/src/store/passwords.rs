//! What password hashing and failed sign-ins may cost the server: argon2 runs
//! a few at a time behind a bounded queue with a share per address, and failed
//! checks lock out an (email, address) pair long before they lock out an
//! address or an email on its own, forgetting failures as time passes.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use be_block_protocol::ErrorCode;
use tokio::sync::Semaphore;

use crate::ServerError;

/// Argon2 hashes and checks that run at once.
pub const PASSWORD_CHECKS_AT_ONCE: usize = 4;
/// Hashes and checks that may wait for one of those; past this the server
/// answers busy instead of queueing.
pub const PASSWORD_CHECKS_WAITING: usize = 32;
/// Hashes and checks one address (an IPv6 /64) may have running or waiting.
pub const PASSWORD_CHECKS_PER_ADDRESS: usize = 2;

/// Failures an (email, address) pair makes before it is locked out.
pub const FREE_FAILURES: u32 = 5;
/// Failures one address makes, over every email, before it is locked out.
pub const FREE_FAILURES_PER_ADDRESS: u32 = 20;
/// Failures one email collects, from every address, before it is locked out
/// everywhere: far above what one stranger's address reaches by itself.
pub const FREE_FAILURES_PER_EMAIL: u32 = 100;
pub const FIRST_LOCKOUT: Duration = Duration::from_secs(30);
pub const LONGEST_LOCKOUT: Duration = Duration::from_secs(60 * 60);
/// One failure is forgotten for each of these that passes without another.
pub const FAILURE_DECAY: Duration = Duration::from_secs(15 * 60);
/// Failure records held; past this the forgotten ones and then the oldest go.
pub const MAX_FAILURE_RECORDS: usize = 10_000;

fn busy() -> ServerError {
    ServerError::Refused(
        ErrorCode::InvalidRequest,
        "The server is busy. Try again in a moment.".into(),
    )
}

/// The part of a client's address limits count by: the address itself, or
/// for IPv6 its /64, which one host or network usually holds whole.
pub fn address_key(address: &str) -> String {
    match address.parse::<IpAddr>() {
        Ok(IpAddr::V6(v6)) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => {
                let segments = v6.segments();
                format!(
                    "{:x}:{:x}:{:x}:{:x}::/64",
                    segments[0], segments[1], segments[2], segments[3]
                )
            }
        },
        Ok(IpAddr::V4(v4)) => v4.to_string(),
        Err(_) => address.to_owned(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum FailureKey {
    Pair(String, String),
    Address(String),
    Email(String),
}

impl FailureKey {
    fn free(&self) -> u32 {
        match self {
            FailureKey::Pair(..) => FREE_FAILURES,
            FailureKey::Address(_) => FREE_FAILURES_PER_ADDRESS,
            FailureKey::Email(_) => FREE_FAILURES_PER_EMAIL,
        }
    }
}

struct Failures {
    count: u32,
    last: Instant,
    until: Option<Instant>,
}

impl Failures {
    fn count_at(&self, now: Instant) -> u32 {
        let decayed = now.saturating_duration_since(self.last).as_secs() / FAILURE_DECAY.as_secs();
        self.count
            .saturating_sub(decayed.try_into().unwrap_or(u32::MAX))
    }

    fn forgotten_at(&self, now: Instant) -> bool {
        self.count_at(now) == 0 && self.until.is_none_or(|until| until <= now)
    }
}

#[derive(Default)]
struct Load {
    total: usize,
    by_address: HashMap<String, usize>,
}

pub(crate) struct Passwords {
    hashing: Arc<Semaphore>,
    load: Arc<Mutex<Load>>,
    failures: Mutex<HashMap<FailureKey, Failures>>,
}

/// A place in the hashing queue, given back when dropped.
pub(crate) struct Admitted {
    load: Arc<Mutex<Load>>,
    address: String,
}

impl Drop for Admitted {
    fn drop(&mut self) {
        let mut load = self.load.lock().unwrap();
        load.total -= 1;
        if let Some(count) = load.by_address.get_mut(&self.address) {
            *count -= 1;
            if *count == 0 {
                load.by_address.remove(&self.address);
            }
        }
    }
}

impl Default for Passwords {
    fn default() -> Self {
        Self {
            hashing: Arc::new(Semaphore::new(PASSWORD_CHECKS_AT_ONCE)),
            load: Arc::default(),
            failures: Mutex::default(),
        }
    }
}

impl Passwords {
    /// Takes a place in the queue for `address`, or refuses at once when the
    /// queue or that address's share of it is full.
    pub(crate) fn admit(&self, address: &str) -> Result<Admitted, ServerError> {
        let address = address_key(address);
        let mut load = self.load.lock().unwrap();
        let mine = load.by_address.get(&address).copied().unwrap_or(0);
        if load.total >= PASSWORD_CHECKS_AT_ONCE + PASSWORD_CHECKS_WAITING
            || mine >= PASSWORD_CHECKS_PER_ADDRESS
        {
            return Err(busy());
        }
        load.total += 1;
        *load.by_address.entry(address.clone()).or_default() += 1;
        Ok(Admitted {
            load: Arc::clone(&self.load),
            address,
        })
    }

    /// Runs `work` on a blocking thread once admitted and a hashing slot is
    /// free. The slot and the place are held until `work` finishes, even if
    /// the caller stops waiting.
    pub(crate) async fn run<T: Send + 'static>(
        &self,
        address: &str,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T, ServerError> {
        let admitted = self.admit(address)?;
        let permit = Arc::clone(&self.hashing)
            .acquire_owned()
            .await
            .map_err(|_| ServerError::Corrupt)?;
        tokio::task::spawn_blocking(move || {
            let result = work();
            drop((permit, admitted));
            result
        })
        .await
        .map_err(|_| ServerError::Corrupt)
    }

    fn keys(email: &str, address: &str) -> [FailureKey; 3] {
        let address = address_key(address);
        [
            FailureKey::Pair(email.to_owned(), address.clone()),
            FailureKey::Address(address),
            FailureKey::Email(email.to_owned()),
        ]
    }

    /// How long sign-ins for `email` from `address` stay refused.
    pub(crate) fn locked_out(&self, email: &str, address: &str, now: Instant) -> Option<Duration> {
        let failures = self.failures.lock().unwrap();
        Self::keys(email, address)
            .iter()
            .filter_map(|key| failures.get(key)?.until?.checked_duration_since(now))
            .filter(|wait| !wait.is_zero())
            .max()
    }

    pub(crate) fn failed(&self, email: &str, address: &str, now: Instant) {
        let mut failures = self.failures.lock().unwrap();
        for key in Self::keys(email, address) {
            if !failures.contains_key(&key) && failures.len() >= MAX_FAILURE_RECORDS {
                make_room(&mut failures, now);
            }
            let free = key.free();
            let entry = failures.entry(key).or_insert(Failures {
                count: 0,
                last: now,
                until: None,
            });
            entry.count = entry.count_at(now) + 1;
            entry.last = now;
            if entry.count >= free {
                let doublings = (entry.count - free).min(16);
                let wait = FIRST_LOCKOUT
                    .saturating_mul(1 << doublings)
                    .min(LONGEST_LOCKOUT);
                entry.until = Some(now + wait);
                tracing::warn!(
                    email,
                    address,
                    failures = entry.count,
                    ?wait,
                    "sign-ins locked out"
                );
            }
        }
    }

    /// The right password from `address`: forgets that pair's and the
    /// email's failures, but not the address's, which may be shared.
    pub(crate) fn succeeded(&self, email: &str, address: &str) {
        let [pair, _, email] = Self::keys(email, address);
        let mut failures = self.failures.lock().unwrap();
        failures.remove(&pair);
        failures.remove(&email);
    }

    /// A reset through the account's own mail forgets every failure for it.
    pub(crate) fn forget_email(&self, email: &str) {
        self.failures.lock().unwrap().retain(|key, _| match key {
            FailureKey::Pair(found, _) | FailureKey::Email(found) => found != email,
            FailureKey::Address(_) => true,
        });
    }

    #[cfg(test)]
    pub(crate) fn records(&self) -> usize {
        self.failures.lock().unwrap().len()
    }
}

fn make_room(failures: &mut HashMap<FailureKey, Failures>, now: Instant) {
    failures.retain(|_, failures| !failures.forgotten_at(now));
    if failures.len() >= MAX_FAILURE_RECORDS
        && let Some(oldest) = failures
            .iter()
            .min_by_key(|(_, failures)| failures.last)
            .map(|(key, _)| key.clone())
    {
        failures.remove(&oldest);
    }
}
