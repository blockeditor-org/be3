//! What edits made on this device announce (`be_block::Announcement`), kept beside the journal
//! entry or outbox commit that holds each edit until the server holds it, then sent once
//! (guides/type_modules.md, "Announcements"). The text is sealed with the block's key, as the
//! journal is.

use std::collections::{BTreeMap, HashSet};

use be_block_commit::CommitId;
use be_block_store::StoreError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Refs, Row, Table, row};

/// How many sent announcements a block remembers, so an edit made again (a replica restarted
/// with it) is not told twice.
const REMEMBERED: usize = 256;

/// What has to happen before an announcement is sent.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum Until {
    /// The edit is in this device's journal as `sequence`, and the server does not hold it yet.
    Journal(u64),
    /// The edit is in this commit, kept in the outbox.
    Commit(CommitId),
    /// The server holds the edit.
    Held,
    /// Sent, or given up: remembered so it is not sent again.
    Sent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Waiting {
    pub(crate) order: u64,
    pub(crate) until: Until,
    pub(crate) made: Made,
}

/// An announcement as this device keeps it: its text sealed with the block's key.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Made {
    pub(crate) key: String,
    #[serde(with = "serde_bytes")]
    pub(crate) sealed: Vec<u8>,
    /// The text for several sent together (`be_block::Announcement::several`).
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
    /// Keeps announcements of edits of `block` until `until`. A key already held, sent or kept
    /// with a commit is not kept again; one waiting in the journal is replaced, since the edit
    /// was made again (a replica restarted with it) and the journal entry may never be held.
    pub fn add_announcements(
        &self,
        block: Uuid,
        until: Until,
        announcements: Vec<Made>,
    ) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut guard = self.state.lock().unwrap();
            let state = &mut *guard;
            for made in announcements {
                let known = state
                    .announcements
                    .get(&block)
                    .and_then(|held| held.get(&made.key));
                if known.is_some_and(|known| !matches!(known.until, Until::Journal(_))) {
                    continue;
                }
                let key = key_of(block, &made.key);
                let waiting = Waiting {
                    order: state.take(),
                    until,
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

    /// Moves the announcements of `block` that `change` answers for to what it answers: `None`
    /// forgets one.
    fn update_announcements(
        &self,
        block: Uuid,
        change: impl Fn(&str, Until) -> Option<Option<Until>>,
    ) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            let Some(held) = state.announcements.get_mut(&block) else {
                return Ok(());
            };
            let mut gone = Vec::new();
            for (key, waiting) in held.iter_mut() {
                match change(key, waiting.until) {
                    None => {}
                    Some(None) => gone.push(key.clone()),
                    Some(Some(until)) if until != waiting.until => {
                        waiting.until = until;
                        rows.push(row(Table::Announcement, key_of(block, key), &*waiting));
                    }
                    Some(Some(_)) => {}
                }
            }
            for key in gone {
                held.remove(&key);
                rows.push(Row {
                    table: Table::Announcement,
                    key: key_of(block, &key),
                    value: None,
                });
            }
        }
        self.write(rows)
    }

    /// The server took the journal entries holding the announcements named.
    pub fn journal_held(&self, block: Uuid, keys: &HashSet<String>) -> Result<(), StoreError> {
        self.update_announcements(block, |key, until| match until {
            Until::Journal(_) if keys.contains(key) => Some(Some(Until::Held)),
            _ => None,
        })
    }

    /// The journal of `block` started over from a commit holding everything up to `sequence`:
    /// what waited in it goes with that commit when this device sealed it (`own`), kept in the
    /// outbox (`kept`) or already on the server; anything else the journal held is gone.
    pub fn journal_rebased(
        &self,
        block: Uuid,
        commit: Option<CommitId>,
        sequence: u64,
        own: bool,
        kept: bool,
    ) -> Result<(), StoreError> {
        self.update_announcements(block, |_, until| match until {
            Until::Journal(at) => Some(match commit {
                Some(commit) if own && at <= sequence => Some(match kept {
                    true => Until::Commit(commit),
                    false => Until::Held,
                }),
                _ => None,
            }),
            _ => None,
        })
    }

    /// The server holds `commits` of `block`: what waited on them is held.
    pub fn commits_held(&self, block: Uuid, commits: &HashSet<CommitId>) -> Result<(), StoreError> {
        self.update_announcements(block, |_, until| match until {
            Until::Commit(commit) if commits.contains(&commit) => Some(Some(Until::Held)),
            _ => None,
        })
    }

    /// The server refused `commits` of `block` kept on this device, or they were given up: what
    /// they announce is never sent.
    pub fn commits_refused(
        &self,
        block: Uuid,
        commits: &HashSet<CommitId>,
    ) -> Result<(), StoreError> {
        self.update_announcements(block, |_, until| match until {
            Until::Commit(commit) if commits.contains(&commit) => Some(None),
            _ => None,
        })
    }

    /// Takes every announcement whose edit the server holds, marking it sent, oldest first.
    pub fn take_held_announcements(&self) -> Result<Vec<Announcing>, StoreError> {
        let mut taken = Vec::new();
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            for (block, held) in state.announcements.iter_mut() {
                for (key, waiting) in held.iter_mut() {
                    if waiting.until != Until::Held {
                        continue;
                    }
                    waiting.until = Until::Sent;
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
                    .filter(|(_, waiting)| waiting.until == Until::Sent)
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
        for (block, keys) in by_block {
            self.update_announcements(block, |key, until| match until {
                Until::Sent if keys.contains(key) => Some(Some(Until::Held)),
                _ => None,
            })?;
        }
        Ok(())
    }

    /// Whether any announcement waits to be sent.
    pub fn has_held_announcements(&self) -> bool {
        self.state
            .lock()
            .unwrap()
            .announcements
            .values()
            .any(|held| held.values().any(|waiting| waiting.until == Until::Held))
    }
}
