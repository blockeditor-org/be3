//! Who a connection comes from, and how much each address may hold at once.

use std::{
    collections::{BTreeMap, HashMap},
    net::{IpAddr, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use tokio::sync::Notify;

/// The address a request comes from: its peer, or, when the peer is a trusted proxy, the
/// nearest address in its `X-Forwarded-For` that is not itself a trusted proxy. Every
/// per-address limit, here and in the portal, counts by this address's [`address_key`].
pub(crate) fn client_address(peer: IpAddr, forwarded: Option<&str>, trusted: &[IpAddr]) -> IpAddr {
    let mut nearest = peer.to_canonical();
    if !trusted.contains(&nearest) {
        return nearest;
    }
    for hop in forwarded.unwrap_or_default().rsplit(',') {
        let hop = hop.trim();
        let Some(hop) = hop
            .parse::<IpAddr>()
            .ok()
            .or_else(|| hop.parse::<SocketAddr>().ok().map(|address| address.ip()))
        else {
            break;
        };
        nearest = hop.to_canonical();
        if !trusted.contains(&nearest) {
            break;
        }
    }
    nearest
}

/// What per-address limits count by: an IPv4 address, or an IPv6 address's /64, since one
/// subscriber is usually given a whole /64.
pub(crate) fn address_key(address: IpAddr) -> IpAddr {
    match address.to_canonical() {
        IpAddr::V6(address) => IpAddr::V6((u128::from(address) & !((1u128 << 64) - 1)).into()),
        address => address,
    }
}

/// Counts what each address holds, up to `limit` each.
pub(crate) struct PerAddress {
    held: Mutex<HashMap<IpAddr, usize>>,
    limit: usize,
}

/// One of an address's places, given back when dropped.
pub(crate) struct Held {
    of: Arc<PerAddress>,
    key: IpAddr,
}

impl PerAddress {
    pub(crate) fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self {
            held: Mutex::new(HashMap::new()),
            limit,
        })
    }

    /// A place for `address`, or none while it already holds `limit`.
    pub(crate) fn take(self: &Arc<Self>, address: IpAddr) -> Option<Held> {
        let key = address_key(address);
        let mut held = self.held.lock().unwrap();
        let count = held.entry(key).or_default();
        if *count >= self.limit {
            return None;
        }
        *count += 1;
        Some(Held {
            of: Arc::clone(self),
            key,
        })
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        let mut held = self.of.held.lock().unwrap();
        if let Some(count) = held.get_mut(&self.key) {
            *count -= 1;
            if *count == 0 {
                held.remove(&self.key);
            }
        }
    }
}

#[cfg(test)]
mod tests;

/// What limits on clients that are not signed in count by: an IPv4 address, or an IPv6
/// address's /48, since one site is often given a whole /48 and anyone may take many /64s.
pub(crate) fn unsigned_key(address: IpAddr) -> IpAddr {
    match address.to_canonical() {
        IpAddr::V6(address) => IpAddr::V6((u128::from(address) & !((1u128 << 80) - 1)).into()),
        address => address,
    }
}

/// Whether a client that is not signed in was let go of to make room for a newer one.
#[derive(Default)]
pub(crate) struct Eviction {
    evicted: AtomicBool,
    notify: Notify,
}

impl Eviction {
    pub(crate) fn evicted(&self) -> bool {
        self.evicted.load(Ordering::Acquire)
    }

    /// Waits until the client is evicted.
    pub(crate) async fn wait(&self) {
        loop {
            let notified = self.notify.notified();
            if self.evicted() {
                return;
            }
            notified.await;
        }
    }

    pub(crate) fn evict(&self) {
        self.evicted.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }
}

/// Clients that are not signed in yet: connections, or HTTP sessions. They are counted apart
/// from the rest and held to a fraction of the room, so they never take what signed-in clients
/// need. One address (see [`unsigned_key`]) holds at most `per_address`; admitting one past
/// `limit` evicts the oldest, so however many there are a new client still gets in to sign in.
pub(crate) struct Unsigned {
    held: Mutex<UnsignedHeld>,
    limit: usize,
    per_address: usize,
}

#[derive(Default)]
struct UnsignedHeld {
    next: u64,
    order: BTreeMap<u64, (IpAddr, Arc<Eviction>)>,
    by_address: HashMap<IpAddr, usize>,
}

impl UnsignedHeld {
    fn remove(&mut self, id: u64) -> Option<Arc<Eviction>> {
        let (key, eviction) = self.order.remove(&id)?;
        if let Some(count) = self.by_address.get_mut(&key) {
            *count -= 1;
            if *count == 0 {
                self.by_address.remove(&key);
            }
        }
        Some(eviction)
    }
}

/// A place among the clients that are not signed in, given back when dropped: once the client
/// signs in, or ends.
pub(crate) struct Anonymous {
    of: Arc<Unsigned>,
    id: u64,
}

impl Unsigned {
    pub(crate) fn new(limit: usize, per_address: usize) -> Arc<Self> {
        Arc::new(Self {
            held: Mutex::new(UnsignedHeld::default()),
            limit,
            per_address,
        })
    }

    /// A place for a client from `address` that `eviction` tells when it is let go of, or none
    /// while the address already holds `per_address`.
    pub(crate) fn admit(
        self: &Arc<Self>,
        address: IpAddr,
        eviction: &Arc<Eviction>,
    ) -> Option<Anonymous> {
        let key = unsigned_key(address);
        let mut held = self.held.lock().unwrap();
        if held.by_address.get(&key).copied().unwrap_or(0) >= self.per_address {
            return None;
        }
        while held.order.len() >= self.limit {
            let Some((&oldest, _)) = held.order.first_key_value() else {
                break;
            };
            if let Some(evicted) = held.remove(oldest) {
                evicted.evict();
            }
        }
        let id = held.next;
        held.next += 1;
        held.order.insert(id, (key, Arc::clone(eviction)));
        *held.by_address.entry(key).or_default() += 1;
        Some(Anonymous {
            of: Arc::clone(self),
            id,
        })
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.held.lock().unwrap().order.len()
    }
}

impl Drop for Anonymous {
    fn drop(&mut self) {
        self.of.held.lock().unwrap().remove(self.id);
    }
}

/// Counts what each address (see [`unsigned_key`]) does in each `window`, up to `limit`.
pub(crate) struct Rate {
    held: Mutex<RateHeld>,
    limit: u32,
    window: std::time::Duration,
}

#[derive(Default)]
struct RateHeld {
    counts: HashMap<IpAddr, (tokio::time::Instant, u32)>,
    /// How many addresses were counted after the last pass dropping finished windows; the next
    /// pass waits until there are twice as many, so passes cost O(1) a call on average.
    pruned: usize,
}

/// The fewest addresses counted before finished windows are dropped.
const RATE_PRUNE_AT: usize = 1024;

impl Rate {
    pub(crate) fn new(limit: u32, window: std::time::Duration) -> Self {
        Self {
            held: Mutex::new(RateHeld::default()),
            limit,
            window,
        }
    }

    /// Counts one more for `address`, or answers false once it did `limit` in this window.
    pub(crate) fn allow(&self, address: IpAddr) -> bool {
        let now = tokio::time::Instant::now();
        let mut held = self.held.lock().unwrap();
        if held.counts.len() >= (2 * held.pruned).max(RATE_PRUNE_AT) {
            let window = self.window;
            held.counts.retain(|_, (start, _)| *start + window > now);
            held.pruned = held.counts.len();
        }
        let (start, count) = held.counts.entry(unsigned_key(address)).or_insert((now, 0));
        if *start + self.window <= now {
            *start = now;
            *count = 0;
        }
        if *count >= self.limit {
            return false;
        }
        *count += 1;
        true
    }
}
