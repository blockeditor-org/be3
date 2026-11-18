//! What the server lacks of the edits made on this device, for editors that show what is still
//! sending (`ToEditor::Lacking`) and for announcing them (`LiveEdit::announcements`). The replica
//! adds each edit as it makes it and drops it once the server holds it: a submission the server
//! numbered, or a commit published to it. Entries outlive
//! the replica: a replica reopened, or opened again after its editors closed, carries on with what
//! the last one lacked.

use std::collections::{BTreeMap, BTreeSet};

use be_block_commit::CommitId;
use be_block_content::Announcement;
use be_sdk_type_api::{EditorId, KeptCommits};

use crate::editor::{Lacked, ToEditor};

/// An edit's bytes past this are not kept or sent: an editor sees only that something is
/// sending, rather than a copy of a paste too large to relay.
const LARGEST_KEPT: usize = 1024 * 1024;

/// What has to happen before the server holds an edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Until {
    /// Made here and not yet numbered: waiting to be submitted, or submitted as the submission
    /// with this counter.
    Unsettled(Option<u64>),
    /// Numbered in the order as `sequence` by the submission `counter` (none for an entry this
    /// device's journal held when the replica opened), until the server says it holds it or a
    /// seal holding it reaches the server: numbered offline, the server lacks it.
    Numbered { counter: Option<u64>, sequence: u64 },
    /// The order passed over it (based before a barrier, or unreadable): it will never land, and
    /// only waits to be dismissed.
    Dropped,
    /// In a commit kept in this device's outbox, until the host says that commit, or one kept
    /// after it, `drained`. None for one kept by an earlier replica that named no commit.
    Kept(Option<CommitId>),
    /// From an earlier replica of the block, until the order holds it or a seal or replacement
    /// reaches the server (this replica's content holds it).
    Carried,
}

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    id: u64,
    operation: Vec<u8>,
    until: Until,
    failed: bool,
    /// The sequence it had in the order, once it had one: how a replayed journal entry is told
    /// from another with the same (or no) bytes.
    sequence: Option<u64>,
    /// What the edit announces, until it goes with the submission holding it.
    announcements: Vec<Announcement>,
}

#[derive(Default)]
pub(crate) struct Carried {
    pub(crate) entries: Vec<Entry>,
    pub(crate) followers: BTreeSet<EditorId>,
    /// The refused commits whose refusal the replicas already applied.
    pub(crate) refused: BTreeSet<CommitId>,
    /// The commits kept in the outbox, oldest first.
    pub(crate) kept: Vec<CommitId>,
    /// The snapshot the replica told each editor last (`Replica::snapshots_shown`).
    pub(crate) snapshots_shown: BTreeMap<EditorId, be_block_store::Hash>,
}

#[derive(Default)]
pub(crate) struct Lacking {
    entries: Vec<Entry>,
    /// The refused commits whose refusal was applied (by a drain or on opening): a commit the
    /// device still holds after it was refused is not refused again.
    refused: BTreeSet<CommitId>,
    /// The commits this replica's edits were kept in, in the order they were kept: each drains
    /// with every one kept before it, since the outbox publishes in order.
    kept: Vec<CommitId>,
    next: u64,
    /// The entries the followers were last told of, by id, with whether each had failed.
    told: Vec<(u64, bool)>,
    /// Something changed since the followers were last told.
    changed: bool,
    followers: BTreeSet<EditorId>,
    fresh: BTreeSet<EditorId>,
}

impl Lacking {
    /// Adds an edit made here, with what it announces.
    pub(crate) fn add(&mut self, mut operation: Vec<u8>, announcements: Vec<Announcement>) {
        if operation.len() > LARGEST_KEPT {
            operation = Vec::new();
        }
        // An edit carried from the replica before, made again here (the host replays what it
        // kept), is followed as this one.
        self.journaled(&operation, None);
        self.push(
            operation,
            Until::Unsettled(None),
            false,
            None,
            announcements,
        );
    }

    fn push(
        &mut self,
        operation: Vec<u8>,
        until: Until,
        failed: bool,
        sequence: Option<u64>,
        announcements: Vec<Announcement>,
    ) {
        self.next += 1;
        let sequence = match until {
            Until::Numbered { sequence, .. } => Some(sequence),
            _ => sequence,
        };
        self.entries.push(Entry {
            id: self.next,
            operation,
            until,
            failed,
            sequence,
            announcements,
        });
        self.changed = true;
    }

    /// What commits kept in the outbox add over the server's head, for a replica that carried
    /// nothing from an earlier one (a host or module restarted): sending until the host says
    /// they `drained`.
    pub(crate) fn kept_unsent(&mut self, operations: Vec<Vec<u8>>, tip: CommitId) {
        if !self.entries.is_empty() {
            return;
        }
        // Kept up to the outbox's last commit, which a drain names once all of it is held.
        let until = self.keeping(tip);
        for mut operation in operations {
            if operation.len() > LARGEST_KEPT {
                operation = Vec::new();
            }
            self.push(operation, until, false, None, Vec::new());
        }
    }

    /// Adds an edit replayed from this device's own journal, numbered offline, which the server
    /// lacks, whether an editor follows yet or not: it opens before they ask.
    pub(crate) fn add_replayed(&mut self, operation: Vec<u8>, sequence: u64) {
        self.journaled(&operation, Some(sequence));
        let until = Until::Numbered {
            counter: None,
            sequence,
        };
        self.push(operation, until, false, None, Vec::new());
    }

    /// The server's journal holds `operation` at `sequence`, or this replica made it again: one
    /// edit carried from before with that sequence, or else with those bytes (when it has any),
    /// is followed no longer. Held by the server's journal, what it announces is announced; made
    /// again, the new edit announces it.
    pub(crate) fn journaled(&mut self, operation: &[u8], sequence: Option<u64>) {
        let named = |entry: &Entry| {
            entry.until == Until::Carried
                && ((sequence.is_some() && entry.sequence == sequence)
                    || (!operation.is_empty() && entry.operation == operation))
        };
        if let Some(at) = self.entries.iter().position(named) {
            self.entries.remove(at);
            self.changed = true;
        }
    }

    /// The next `count` edits not yet submitted go as the submission `counter`: answers what
    /// they announce, which the host keeps from here on.
    pub(crate) fn submitted(&mut self, counter: u64, count: u64) -> Vec<Announcement> {
        let mut taken = Vec::new();
        let mut left = count;
        for entry in &mut self.entries {
            if left == 0 {
                break;
            }
            if entry.until == Until::Unsettled(None) {
                entry.until = Until::Unsettled(Some(counter));
                taken.append(&mut entry.announcements);
                left -= 1;
            }
        }
        taken
    }

    /// What was submitted and not numbered goes again in new submissions.
    pub(crate) fn unsubmitted(&mut self) {
        for entry in &mut self.entries {
            if let Until::Unsettled(Some(_)) = entry.until {
                entry.until = Until::Unsettled(None);
            }
        }
    }

    /// The order numbered the submission `counter` as `sequence`.
    pub(crate) fn numbered(&mut self, counter: u64, sequence: u64) {
        for entry in &mut self.entries {
            if entry.until == Until::Unsettled(Some(counter)) {
                entry.until = Until::Numbered {
                    counter: Some(counter),
                    sequence,
                };
                entry.sequence = Some(sequence);
            }
        }
    }

    /// The server holds the submission `counter`.
    pub(crate) fn held(&mut self, counter: u64) {
        self.update(|until| match until {
            Until::Unsettled(Some(held))
            | Until::Numbered {
                counter: Some(held),
                ..
            } if held == counter => None,
            until => Some(until),
        });
    }

    /// An edit made here that can never land (made before a restart this replica opened after):
    /// shown as failed until it is dismissed.
    pub(crate) fn add_dropped(&mut self, mut operation: Vec<u8>) {
        if operation.len() > LARGEST_KEPT {
            operation = Vec::new();
        }
        self.push(operation, Until::Dropped, true, None, Vec::new());
    }

    /// The order passed over the submission `counter`: its edits fail.
    pub(crate) fn dropped(&mut self, counter: u64) {
        for entry in &mut self.entries {
            if matches!(entry.until, Until::Unsettled(Some(held)) | Until::Numbered { counter: Some(held), .. } if held == counter)
            {
                entry.until = Until::Dropped;
                entry.failed = true;
                self.changed = true;
            }
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

    /// A seal holding the order up to `through` was published (`held`) or kept as `commit`.
    pub(crate) fn sealed(&mut self, through: u64, held: bool, commit: CommitId) {
        let done = match held {
            true => None,
            false => Some(self.keeping(commit)),
        };
        self.update(|until| match until {
            Until::Numbered { sequence, .. } if sequence <= through => done,
            Until::Carried => done,
            until => Some(until),
        });
    }

    /// A replacement landed: what this replica had not had numbered is in the stash it kept of
    /// it, never to be numbered itself.
    pub(crate) fn replaced(&mut self) {
        self.update(|until| match until {
            Until::Unsettled(_) | Until::Carried => None,
            until => Some(until),
        });
    }

    /// Notes `commit` as kept in the outbox, after every one kept before it.
    fn keeping(&mut self, commit: CommitId) -> Until {
        if !self.kept.contains(&commit) {
            self.kept.push(commit);
        }
        Until::Kept(Some(commit))
    }

    /// The kept commits up to the last of `drained` this replica knows reached the server, and
    /// so did any kept by an earlier replica that named no commit: those edits are held. One
    /// the server refused stays until it is dismissed, so a "Not sent" never goes unseen.
    fn forget_kept(&mut self, drained: &[CommitId]) {
        let reached = drained
            .iter()
            .filter_map(|commit| self.kept.iter().position(|kept| kept == commit))
            .max()
            .map_or_else(Vec::new, |last| self.kept.drain(..=last).collect());
        let before = self.entries.len();
        self.entries.retain(|entry| {
            let gone = match entry.until {
                Until::Kept(None) => true,
                Until::Kept(Some(commit)) => reached.contains(&commit),
                _ => false,
            };
            !gone || entry.failed
        });
        self.changed |= self.entries.len() != before;
    }

    /// The kept commits up to those named reached the server, or it refused them: those edits
    /// then show as failed rather than sending for ever.
    pub(crate) fn drained(&mut self, refused: bool, commits: &[CommitId]) {
        if !refused {
            return self.forget_kept(commits);
        }
        self.refused.extend(commits.iter().copied());
        // Edits carried from an earlier replica may be among the refused commits, and
        // nothing else would ever settle them.
        for entry in &mut self.entries {
            if matches!(entry.until, Until::Kept(_) | Until::Carried) && !entry.failed {
                entry.until = Until::Kept(None);
                entry.failed = true;
                self.changed = true;
            }
        }
    }

    /// The replica opened knowing what commits of the block this device still keeps: what an
    /// earlier replica kept reached the server meanwhile, or was refused. A refused commit
    /// stays on the device after its edit is dismissed, so only one not applied before fails
    /// what was kept.
    pub(crate) fn opened_with(&mut self, kept: KeptCommits, refused: &[CommitId]) {
        let new: Vec<CommitId> = refused
            .iter()
            .copied()
            .filter(|commit| !self.refused.contains(commit))
            .collect();
        if !new.is_empty() {
            self.drained(true, &new);
        }
        match kept {
            KeptCommits::None => {
                let kept = std::mem::take(&mut self.kept);
                for entry in &mut self.entries {
                    if matches!(entry.until, Until::Kept(Some(_))) {
                        entry.until = Until::Kept(None);
                    }
                }
                self.forget_kept(&kept);
            }
            KeptCommits::Outbox => {}
        }
    }

    /// The person let an edit that failed go, whether they sent it again or not.
    pub(crate) fn dismiss(&mut self, id: u64) {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.id != id || !entry.failed);
        self.changed |= self.entries.len() != before;
    }

    /// What the next replica of the block carries on with: the edits, and the editors that
    /// follow them, which a replica reopened in place keeps attached.
    pub(crate) fn carry(&mut self) -> Carried {
        let entries = std::mem::take(&mut self.entries)
            .into_iter()
            // What the order passed over is in no content the next replica loads, so nothing
            // could show it: it goes with this replica.
            .filter(|entry| entry.until != Until::Dropped)
            .map(|entry| Entry {
                sequence: match entry.until {
                    Until::Numbered { sequence, .. } => Some(sequence),
                    _ => entry.sequence,
                },
                until: match entry.until {
                    Until::Kept(commit) => Until::Kept(commit),
                    _ => Until::Carried,
                },
                ..entry
            })
            .collect();
        Carried {
            entries,
            followers: std::mem::take(&mut self.followers),
            refused: std::mem::take(&mut self.refused),
            kept: std::mem::take(&mut self.kept),
            snapshots_shown: BTreeMap::new(),
        }
    }

    pub(crate) fn adopt(&mut self, carried: Carried) {
        self.refused.extend(carried.refused);
        self.kept.extend(carried.kept);
        for entry in carried.entries {
            self.push(
                entry.operation,
                entry.until,
                entry.failed,
                entry.sequence,
                entry.announcements,
            );
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

#[cfg(test)]
mod tests;
