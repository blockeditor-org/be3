//! Who a connection comes from, and how much each address may hold at once.

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
};

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
