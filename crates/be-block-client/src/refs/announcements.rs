//! What log entries appended on this device announce (`be_block_content::Along::announcement`),
//! kept from when the server holds the entry until they are sent, once (guides/type_modules.md,
//! "Announcements"). The text is sealed with the block's key, as the log is.

use std::collections::{BTreeMap, HashSet};

use be_block_store::StoreError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Refs, Row, Table, row};

/// How many sent announcements a block remembers, so an edit made again (a replica restarted
/// with it) is not told twice.
const REMEMBERED: usize = 256;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Waiting {
    pub(crate) order: u64,
    /// Sent, or given up: remembered so it is not sent again. Otherwise the server holds the
    /// entry and it waits to go.
    pub(crate) sent: bool,
    pub(crate) made: Made,
}

/// An announcement as this device keeps it: its text sealed with the block's key.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Made {
    pub(crate) key: String,
    #[serde(with = "serde_bytes")]
    pub(crate) sealed: Vec<u8>,
    /// The text for several sent together (`be_block_content::Announcement::several`).
    pub(crate) several: String,
    pub(crate) mentions: Vec<Uuid>,
}

/// An announcement the server holds the edit of, to send.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Announcing {
    pub(crate) block: Uuid,
    pub(crate) made: Made,
}

fn key_of(block: Uuid, key: &str) -> Vec<u8> {
    let mut bytes = block.as_bytes().to_vec();
    bytes.extend(key.as_bytes());
    bytes
}

/// Forgets every announcement of `block`: this device no longer keeps the block.
pub(super) fn forget(state: &mut super::State, block: Uuid, rows: &mut Vec<Row>) {
    for key in state
        .announcements
        .remove(&block)
        .unwrap_or_default()
        .into_keys()
    {
        rows.push(Row {
            table: Table::Announcement,
            key: key_of(block, &key),
            value: None,
        });
    }
}

impl Refs {
    /// Keeps announcements of entries of `block` the server holds, to send. A key already held
    /// or sent is not kept again.
    pub fn add_announcements(
        &self,
        block: Uuid,
        announcements: Vec<Made>,
    ) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut guard = self.state.lock().unwrap();
            let state = &mut *guard;
            for made in announcements {
                if state
                    .announcements
                    .get(&block)
                    .is_some_and(|held| held.contains_key(&made.key))
                {
                    continue;
                }
                let key = key_of(block, &made.key);
                let waiting = Waiting {
                    order: state.take(),
                    sent: false,
                    made,
                };
                rows.push(row(Table::Announcement, key, &waiting));
                state
                    .announcements
                    .entry(block)
                    .or_default()
                    .insert(waiting.made.key.clone(), waiting);
            }
        }
        self.write(rows)
    }

    /// Takes every announcement waiting to go, marking it sent, oldest first.
    pub fn take_held_announcements(&self) -> Result<Vec<Announcing>, StoreError> {
        let mut taken = Vec::new();
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            for (block, held) in state.announcements.iter_mut() {
                for (key, waiting) in held.iter_mut() {
                    if waiting.sent {
                        continue;
                    }
                    waiting.sent = true;
                    rows.push(row(Table::Announcement, key_of(*block, key), &*waiting));
                    taken.push((
                        waiting.order,
                        Announcing {
                            block: *block,
                            made: waiting.made.clone(),
                        },
                    ));
                }
                let mut sent: Vec<(u64, String)> = held
                    .iter()
                    .filter(|(_, waiting)| waiting.sent)
                    .map(|(key, waiting)| (waiting.order, key.clone()))
                    .collect();
                sent.sort();
                let over = sent.len().saturating_sub(REMEMBERED);
                for (_, key) in sent.into_iter().take(over) {
                    held.remove(&key);
                    rows.push(Row {
                        table: Table::Announcement,
                        key: key_of(*block, &key),
                        value: None,
                    });
                }
            }
        }
        self.write(rows)?;
        taken.sort_by_key(|(order, _)| *order);
        Ok(taken
            .into_iter()
            .map(|(_, announcing)| announcing)
            .collect())
    }

    /// Announcements taken to send that could not go (the server was unreachable): they are
    /// sent next time.
    pub fn unsent_announcements(&self, announcements: &[Announcing]) -> Result<(), StoreError> {
        let mut by_block: BTreeMap<Uuid, HashSet<String>> = BTreeMap::new();
        for announcing in announcements {
            by_block
                .entry(announcing.block)
                .or_default()
                .insert(announcing.made.key.clone());
        }
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            for (block, keys) in by_block {
                let Some(held) = state.announcements.get_mut(&block) else {
                    continue;
                };
                for (key, waiting) in held.iter_mut() {
                    if waiting.sent && keys.contains(key) {
                        waiting.sent = false;
                        rows.push(row(Table::Announcement, key_of(block, key), &*waiting));
                    }
                }
            }
        }
        self.write(rows)
    }

    /// Whether any announcement waits to be sent.
    pub fn has_held_announcements(&self) -> bool {
        self.state
            .lock()
            .unwrap()
            .announcements
            .values()
            .any(|held| held.values().any(|waiting| !waiting.sent))
    }
}
