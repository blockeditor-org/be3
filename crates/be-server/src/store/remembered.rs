//! The addresses each email's owner signs in from, which the code limits trust
//! (`codes::CODES_PER_ACCOUNT`): one that signed in, or finished a registration or recovery
//! from the email's own mail. Bounded, and kept in memory: a restart forgets them.

use std::{collections::HashMap, sync::Mutex, time::Instant};

use crate::address::address_key_text;

/// Addresses remembered for one email; past it the oldest goes.
pub const KNOWN_ADDRESSES_PER_EMAIL: usize = 8;
/// Emails remembered at once; past it the one used longest ago goes.
pub const MAX_KNOWN_EMAILS: usize = 10_000;

#[derive(Default)]
pub(crate) struct Remembered {
    known: Mutex<HashMap<String, Vec<(String, Instant)>>>,
}

impl Remembered {
    /// Remembers `address` as one the owner of `email` uses.
    pub(crate) fn remember(&self, email: &str, address: &str, now: Instant) {
        let address = address_key_text(address);
        let mut known = self.known.lock().unwrap();
        if !known.contains_key(email)
            && known.len() >= MAX_KNOWN_EMAILS
            && let Some(oldest) = known
                .iter()
                .min_by_key(|(_, addresses)| addresses.iter().map(|(_, at)| *at).max())
                .map(|(email, _)| email.clone())
        {
            known.remove(&oldest);
        }
        let addresses = known.entry(email.to_owned()).or_default();
        addresses.retain(|(found, _)| *found != address);
        if addresses.len() >= KNOWN_ADDRESSES_PER_EMAIL {
            let oldest = addresses
                .iter()
                .enumerate()
                .min_by_key(|(_, (_, at))| *at)
                .map(|(index, _)| index)
                .unwrap_or_default();
            addresses.remove(oldest);
        }
        addresses.push((address, now));
    }

    pub(crate) fn known(&self, email: &str, address: &str) -> bool {
        let address = address_key_text(address);
        self.known
            .lock()
            .unwrap()
            .get(email)
            .is_some_and(|addresses| addresses.iter().any(|(found, _)| *found == address))
    }
}
