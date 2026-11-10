//! What the server lacks of the edits made on this device, for editors that show what is still
//! sending (`ToEditor::Lacking`). The replica adds each edit as it makes it and drops it once the
//! server holds it: an append the server answered, a commit published to it, or the owner
//! accepting a follower's edit. Entries outlive the replica: a replica reopened, or opened again
//! after its editors closed, carries on with what the last one lacked.

use std::collections::{BTreeMap, BTreeSet};

use be_sdk_type_api::EditorId;

use crate::editor::{Lacked, ToEditor};

/// What has to happen before the server holds an edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Until {
    /// A follower's edit, until the owner accepts it.
    Unsettled,
    /// An owner's edit held back while a seal is out, until it is sequenced.
    Waiting,
    /// Sequenced, until an append holding it is answered or a seal holds it.
    Sequenced(u64),
    /// Appended to this device's journal alone, until a seal holds it.
    Lost(u64),
    /// Too large to relay, published as a replacement, until that publish is answered.
    Replacing,
    /// In a commit kept in this device's outbox, until the host says it `drained`.
    Kept,
    /// From an earlier replica of the block, until a seal or replacement reaches the server
    /// (this replica's content holds it) or the outbox drains.
    Carried,
}

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    id: u64,
    operation: Vec<u8>,
    until: Until,
    failed: bool,
}

#[derive(Default)]
pub(crate) struct Carried {
    pub(crate) entries: Vec<Entry>,
    pub(crate) followers: BTreeSet<EditorId>,
}

#[derive(Default)]
pub(crate) struct Lacking {
    entries: Vec<Entry>,
    next: u64,
    /// What the followers were last told, by id: whether it had failed.
    told: BTreeMap<u64, bool>,
    followers: BTreeSet<EditorId>,
    fresh: BTreeSet<EditorId>,
}

impl Lacking {
    pub(crate) fn add(&mut self, operation: Vec<u8>, until: Until) {
        // An edit carried from the replica before, made again here (the host replays what it
        // kept), is followed as this one.
        self.entries
            .retain(|entry| entry.until != Until::Carried || entry.operation != operation);
        self.next += 1;
        self.entries.push(Entry {
            id: self.next,
            operation,
            until,
            failed: false,
        });
    }

    /// Adds an edit replayed from this device's own journal, which the server lacks.
    pub(crate) fn add_replayed(&mut self, operation: Vec<u8>, sequence: u64) {
        self.add(operation, Until::Lost(sequence));
    }

    /// The server's journal holds `operation`: an edit carried from before is held.
    pub(crate) fn journaled(&mut self, operation: &[u8]) {
        self.entries
            .retain(|entry| entry.until != Until::Carried || entry.operation != operation);
    }

    fn update(&mut self, change: impl Fn(Until) -> Option<Until>) {
        self.entries.retain_mut(|entry| match change(entry.until) {
            Some(until) => {
                entry.until = until;
                true
            }
            None => false,
        });
    }

    /// The owner sequenced the edit it held back longest as `sequence`.
    pub(crate) fn sequenced(&mut self, sequence: u64) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.until == Until::Waiting)
        {
            entry.until = Until::Sequenced(sequence);
        }
    }

    /// The edits held back went to an owner elsewhere instead.
    pub(crate) fn waiting_to_unsettled(&mut self) {
        self.update(|until| {
            Some(match until {
                Until::Waiting => Until::Unsettled,
                until => until,
            })
        });
    }

    pub(crate) fn appended(&mut self, from: u64, through: u64, held: bool) {
        self.update(|until| match until {
            Until::Sequenced(sequence) if (from..=through).contains(&sequence) => match held {
                true => None,
                false => Some(Until::Lost(sequence)),
            },
            until => Some(until),
        });
    }

    /// A seal holding everything sequenced up to `through` was published (`held`) or kept.
    pub(crate) fn sealed(&mut self, through: u64, held: bool) {
        let done = match held {
            true => None,
            false => Some(Until::Kept),
        };
        self.update(|until| match until {
            Until::Sequenced(sequence) | Until::Lost(sequence) if sequence <= through => done,
            Until::Carried => done,
            until => Some(until),
        });
    }

    /// A replacement holding everything this replica shows was published (`held`) or kept.
    pub(crate) fn replaced(&mut self, held: bool) {
        self.update(|until| match until {
            Until::Replacing | Until::Carried => match held {
                true => None,
                false => Some(Until::Kept),
            },
            until => Some(until),
        });
    }

    /// The kept commits reached the server, or it refused them: those edits then show as
    /// failed rather than sending for ever.
    pub(crate) fn drained(&mut self, refused: bool) {
        match refused {
            false => self.update(|until| match until {
                Until::Kept | Until::Carried => None,
                until => Some(until),
            }),
            true => {
                for entry in &mut self.entries {
                    if matches!(entry.until, Until::Kept | Until::Carried) {
                        entry.failed = true;
                    }
                }
            }
        }
    }

    /// A follower has `unsettled` edits the owner has not accepted, the newest it made: the
    /// older ones it lists as unsettled were accepted.
    pub(crate) fn settled_to(&mut self, unsettled: u64) {
        let listed = self
            .entries
            .iter()
            .filter(|entry| entry.until == Until::Unsettled)
            .count() as u64;
        let mut accepted = listed.saturating_sub(unsettled);
        self.entries.retain(|entry| {
            if accepted > 0 && entry.until == Until::Unsettled {
                accepted -= 1;
                return false;
            }
            true
        });
    }

    /// This replica took the session over: its unaccepted edits are its own to sequence now,
    /// and the next seal holds them.
    pub(crate) fn promoted(&mut self) {
        self.update(|until| {
            Some(match until {
                Until::Unsettled => Until::Carried,
                until => until,
            })
        });
    }

    /// What the next replica of the block carries on with: the edits, and the editors that
    /// follow them, which a replica reopened in place keeps attached.
    pub(crate) fn carry(&mut self) -> Carried {
        let entries = std::mem::take(&mut self.entries)
            .into_iter()
            .map(|entry| Entry {
                until: match entry.until {
                    Until::Kept => Until::Kept,
                    _ => Until::Carried,
                },
                ..entry
            })
            .collect();
        Carried {
            entries,
            followers: std::mem::take(&mut self.followers),
        }
    }

    pub(crate) fn adopt(&mut self, carried: Carried) {
        for entry in carried.entries {
            self.add(entry.operation, entry.until);
            if let Some(added) = self.entries.last_mut() {
                added.failed = entry.failed;
            }
        }
        for editor in carried.followers {
            self.follow(editor);
        }
    }

    pub(crate) fn follow(&mut self, editor: EditorId) {
        self.followers.insert(editor);
        self.fresh.insert(editor);
    }

    pub(crate) fn unfollow(&mut self, editor: EditorId) {
        self.followers.remove(&editor);
        self.fresh.remove(&editor);
    }

    /// What each follower needs to hear since it was last told: everything for one that just
    /// asked, otherwise what was added, failed or held.
    pub(crate) fn messages(&mut self) -> Vec<(EditorId, ToEditor)> {
        let mut messages = Vec::new();
        let now: BTreeMap<u64, bool> = self
            .entries
            .iter()
            .map(|entry| (entry.id, entry.failed))
            .collect();
        let lacked = |ids: &dyn Fn(&Entry) -> bool, entries: &[Entry]| -> Vec<Lacked> {
            entries
                .iter()
                .filter(|entry| ids(entry))
                .map(|entry| Lacked {
                    id: entry.id,
                    operation: entry.operation.clone(),
                    failed: entry.failed,
                })
                .collect()
        };
        let changed = lacked(
            &|entry| self.told.get(&entry.id) != Some(&entry.failed),
            &self.entries,
        );
        let held: Vec<u64> = self
            .told
            .keys()
            .filter(|id| !now.contains_key(id))
            .copied()
            .collect();
        for editor in &self.followers {
            if self.fresh.contains(editor) {
                messages.push((
                    *editor,
                    ToEditor::Lacking {
                        reset: true,
                        lacking: lacked(&|_| true, &self.entries),
                        held: Vec::new(),
                    },
                ));
            } else if !changed.is_empty() || !held.is_empty() {
                messages.push((
                    *editor,
                    ToEditor::Lacking {
                        reset: false,
                        lacking: changed.clone(),
                        held: held.clone(),
                    },
                ));
            }
        }
        self.fresh.clear();
        self.told = now;
        messages
    }
}
