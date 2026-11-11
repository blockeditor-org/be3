//! What the server lacks of the edits made on this device, for editors that show what is still
//! sending (`ToEditor::Lacking`). While an editor follows, the replica adds each edit as it makes
//! it and drops it once the server holds it: an append the server answered, a commit published to
//! it, or the owner accepting a follower's edit. Entries outlive the replica: a replica reopened,
//! or opened again after its editors closed, carries on with what the last one lacked.

use std::collections::BTreeSet;

use be_block_commit::CommitId;
use be_sdk_type_api::{EditorId, KeptCommits};

use crate::editor::{Lacked, ToEditor};

/// An edit's bytes past this are not kept or sent: an editor sees only that something is
/// sending, rather than a copy of a paste too large to relay.
const LARGEST_KEPT: usize = 1024 * 1024;

/// What has to happen before the server holds an edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Until {
    /// A follower's edit, until the owner accepts it.
    Unsettled,
    /// A follower's edit kept in a stash it handed the owner, until the owner says it saved it.
    Handed(CommitId),
    /// An owner's edit held back while a seal is out, until it is sequenced.
    Waiting,
    /// Sequenced, until an append holding it is answered or a seal holds it.
    Sequenced(u64),
    /// Appended to this device's journal alone, until a seal holds it.
    Lost(u64),
    /// Journaled on this device alone while it owned the session, which another device took: no
    /// seal of this replica can hold it, so it only waits to be dismissed.
    Dropped,
    /// Too large to relay, published as a replacement, until that publish is answered.
    Replacing,
    /// In a commit kept in this device's outbox, until the host says it `drained`.
    Kept,
    /// From an earlier replica of the block, until a seal or replacement reaches the server
    /// (this replica's content holds it).
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
    /// The entries the followers were last told of, by id, with whether each had failed.
    told: Vec<(u64, bool)>,
    /// Something changed since the followers were last told.
    changed: bool,
    followers: BTreeSet<EditorId>,
    fresh: BTreeSet<EditorId>,
}

impl Lacking {
    /// Adds an edit made here, while an editor follows.
    pub(crate) fn add(&mut self, mut operation: Vec<u8>, until: Until) {
        if self.followers.is_empty() {
            return;
        }
        if operation.len() > LARGEST_KEPT {
            operation = Vec::new();
        }
        // An edit carried from the replica before, made again here (the host replays what it
        // kept), is followed as this one.
        self.journaled(&operation);
        self.push(operation, until, false);
    }

    fn push(&mut self, operation: Vec<u8>, until: Until, failed: bool) {
        self.next += 1;
        self.entries.push(Entry {
            id: self.next,
            operation,
            until,
            failed,
        });
        self.changed = true;
    }

    /// Adds an edit replayed from this device's own journal, which the server lacks, whether an
    /// editor follows yet or not: it opens before they ask.
    pub(crate) fn add_replayed(&mut self, operation: Vec<u8>, sequence: u64) {
        self.journaled(&operation);
        self.push(operation, Until::Lost(sequence), false);
    }

    /// The server's journal holds `operation`, or this replica made it again: one edit carried
    /// from before with those bytes is followed no longer.
    pub(crate) fn journaled(&mut self, operation: &[u8]) {
        if let Some(at) = self
            .entries
            .iter()
            .position(|entry| entry.until == Until::Carried && entry.operation == operation)
        {
            self.entries.remove(at);
            self.changed = true;
        }
    }

    fn update(&mut self, change: impl Fn(Until) -> Option<Until>) {
        let before = self.entries.len();
        let mut moved = false;
        self.entries.retain_mut(|entry| match change(entry.until) {
            Some(until) => {
                moved |= until != entry.until;
                entry.until = until;
                true
            }
            None => false,
        });
        self.changed |= moved || self.entries.len() != before;
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

    /// A replacement holding everything this replica shows was published (`held`) or kept: a
    /// follower's unaccepted edits are in it too.
    pub(crate) fn replaced(&mut self, held: bool) {
        self.update(|until| match until {
            Until::Replacing | Until::Carried | Until::Unsettled => match held {
                true => None,
                false => Some(Until::Kept),
            },
            until => Some(until),
        });
    }

    /// A follower stashed its unaccepted edits as `commit` and handed it to the owner.
    pub(crate) fn handed(&mut self, commit: CommitId) {
        self.update(|until| {
            Some(match until {
                Until::Unsettled => Until::Handed(commit),
                until => until,
            })
        });
    }

    /// The owner saved the stash it was handed as `commit`.
    pub(crate) fn saved(&mut self, commit: CommitId) {
        self.update(|until| match until {
            Until::Handed(handed) if handed == commit => None,
            until => Some(until),
        });
    }

    /// The kept commits reached the server, or it refused them: those edits then show as
    /// failed rather than sending for ever.
    pub(crate) fn drained(&mut self, refused: bool) {
        match refused {
            false => self.update(|until| match until {
                Until::Kept => None,
                until => Some(until),
            }),
            true => {
                // Edits carried from an earlier replica may be among the refused commits, and
                // nothing else would ever settle them.
                for entry in &mut self.entries {
                    if matches!(entry.until, Until::Kept | Until::Carried) && !entry.failed {
                        entry.until = Until::Kept;
                        entry.failed = true;
                        self.changed = true;
                    }
                }
            }
        }
    }

    /// The replica opened knowing what commits of the block this device still keeps: what an
    /// earlier replica kept reached the server meanwhile, or was refused.
    pub(crate) fn opened_with(&mut self, kept: KeptCommits) {
        match kept {
            KeptCommits::None => self.update(|until| match until {
                Until::Kept => None,
                until => Some(until),
            }),
            KeptCommits::Outbox => {}
            KeptCommits::Refused => self.drained(true),
        }
    }

    /// This replica is a follower now: what it journaled alone as the owner never reached the
    /// session's new owner, and nothing it does can seal it. It fails.
    pub(crate) fn lost(&mut self) {
        for entry in &mut self.entries {
            if matches!(entry.until, Until::Lost(_)) {
                entry.until = Until::Dropped;
                entry.failed = true;
                self.changed = true;
            }
        }
    }

    /// The person let an edit that failed go, whether they sent it again or not.
    pub(crate) fn dismiss(&mut self, id: u64) {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.id != id || !entry.failed);
        self.changed |= self.entries.len() != before;
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
        if accepted == 0 {
            return;
        }
        self.entries.retain(|entry| {
            if accepted > 0 && entry.until == Until::Unsettled {
                accepted -= 1;
                return false;
            }
            true
        });
        self.changed = true;
    }

    /// This replica took the session over: its unaccepted edits are its own to sequence now,
    /// and the next seal holds them.
    pub(crate) fn promoted(&mut self) {
        if self
            .entries
            .iter()
            .any(|entry| entry.until == Until::Unsettled)
        {
            self.update(|until| {
                Some(match until {
                    Until::Unsettled => Until::Carried,
                    until => until,
                })
            });
        }
    }

    /// What the next replica of the block carries on with: the edits, and the editors that
    /// follow them, which a replica reopened in place keeps attached.
    pub(crate) fn carry(&mut self) -> Carried {
        let entries = std::mem::take(&mut self.entries)
            .into_iter()
            .map(|entry| Entry {
                until: match entry.until {
                    Until::Kept => Until::Kept,
                    Until::Dropped => Until::Dropped,
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
            self.push(entry.operation, entry.until, entry.failed);
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
        if self.followers.is_empty() {
            self.told.clear();
            self.fresh.clear();
            return Vec::new();
        }
        if !self.changed && self.fresh.is_empty() {
            return Vec::new();
        }
        self.changed = false;
        let now: Vec<(u64, bool)> = self
            .entries
            .iter()
            .map(|entry| (entry.id, entry.failed))
            .collect();
        let lacked = |entry: &Entry| Lacked {
            id: entry.id,
            operation: entry.operation.clone(),
            failed: entry.failed,
        };
        let told: std::collections::BTreeSet<(u64, bool)> = self.told.iter().copied().collect();
        let kept: std::collections::BTreeSet<u64> = now.iter().map(|(id, _)| *id).collect();
        let changed: Vec<Lacked> = self
            .entries
            .iter()
            .filter(|entry| !told.contains(&(entry.id, entry.failed)))
            .map(lacked)
            .collect();
        let held: Vec<u64> = self
            .told
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| !kept.contains(id))
            .collect();
        let mut messages = Vec::new();
        for editor in &self.followers {
            if self.fresh.contains(editor) {
                messages.push((
                    *editor,
                    ToEditor::Lacking {
                        reset: true,
                        lacking: self.entries.iter().map(lacked).collect(),
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
