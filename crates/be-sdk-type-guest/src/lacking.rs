//! What the server lacks of the edits made on this device, for editors that show what is still
//! sending (`ToEditor::Lacking`) and for announcing them (`LiveEdit::announcements`). The replica
//! adds each edit as it makes it and drops it once the server holds it: an append the server
//! answered, a commit published to it, or the owner accepting a follower's edit. Entries outlive
//! the replica: a replica reopened, or opened again after its editors closed, carries on with what
//! the last one lacked.

use std::collections::BTreeSet;

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
    /// In a commit kept in this device's outbox, until the host says that commit, or one kept
    /// after it, `drained`. None for one kept by an earlier replica that named no commit.
    Kept(Option<CommitId>),
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
    /// The sequence it had in the journal, once it had one: how a replayed journal entry is
    /// told from another with the same (or no) bytes, and from a folded one.
    sequence: Option<u64>,
    /// What the edit announces, until it goes with the append holding it (an owner's) or the
    /// server holds it (a follower's).
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
    /// What the edits the server came to hold since this was last taken announce, which no
    /// append carried: a follower's.
    held: Vec<Announcement>,
    /// The entries the followers were last told of, by id, with whether each had failed.
    told: Vec<(u64, bool)>,
    /// Something changed since the followers were last told.
    changed: bool,
    followers: BTreeSet<EditorId>,
    fresh: BTreeSet<EditorId>,
}

impl Lacking {
    /// Adds an edit made here, with what it announces.
    pub(crate) fn add(
        &mut self,
        mut operation: Vec<u8>,
        until: Until,
        announcements: Vec<Announcement>,
    ) {
        if operation.len() > LARGEST_KEPT {
            operation = Vec::new();
        }
        // An edit carried from the replica before, made again here (the host replays what it
        // kept), is followed as this one.
        self.journaled(&operation, None);
        self.push(operation, until, false, None, announcements);
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
            Until::Sequenced(sequence) | Until::Lost(sequence) => Some(sequence),
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

    /// Adds an edit replayed from this device's own journal, which the server lacks, whether an
    /// editor follows yet or not: it opens before they ask.
    pub(crate) fn add_replayed(&mut self, operation: Vec<u8>, sequence: u64) {
        self.journaled(&operation, Some(sequence));
        self.push(operation, Until::Lost(sequence), false, None, Vec::new());
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
            let entry = self.entries.remove(at);
            if sequence.is_some() {
                self.held.extend(entry.announcements);
            }
            self.changed = true;
        }
    }

    /// What the edits sequenced from `from` through `through` announce, to go with the append
    /// holding them: the host keeps it from here on.
    pub(crate) fn take_appended(&mut self, from: u64, through: u64) -> Vec<(u64, Announcement)> {
        let mut taken = Vec::new();
        for entry in &mut self.entries {
            if let Until::Sequenced(sequence) = entry.until
                && (from..=through).contains(&sequence)
            {
                taken.extend(
                    std::mem::take(&mut entry.announcements)
                        .into_iter()
                        .map(|announcement| (sequence, announcement)),
                );
            }
        }
        taken
    }

    fn update(&mut self, change: impl Fn(Until) -> Option<Until>) {
        let before = self.entries.len();
        let mut moved = false;
        let mut held = Vec::new();
        self.entries.retain_mut(|entry| match change(entry.until) {
            Some(until) => {
                moved |= until != entry.until;
                entry.until = until;
                true
            }
            None => {
                held.append(&mut entry.announcements);
                false
            }
        });
        self.held.extend(held);
        self.changed |= moved || self.entries.len() != before;
    }

    /// What the edits the server came to hold since this was last called announce, as they were
    /// added.
    pub(crate) fn take_held(&mut self) -> Vec<Announcement> {
        std::mem::take(&mut self.held)
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

    /// A seal holding everything sequenced up to `through` was published (`held`) or kept as
    /// `commit`.
    pub(crate) fn sealed(&mut self, through: u64, held: bool, commit: CommitId) {
        let done = match held {
            true => None,
            false => Some(self.keeping(commit)),
        };
        self.update(|until| match until {
            Until::Sequenced(sequence) | Until::Lost(sequence) if sequence <= through => done,
            Until::Carried => done,
            until => Some(until),
        });
    }

    /// A replacement holding everything this replica shows was published (`held`) or kept as
    /// `commit`: a follower's unaccepted edits are in it too.
    pub(crate) fn replaced(&mut self, held: bool, commit: CommitId) {
        let done = match held {
            true => None,
            false => Some(self.keeping(commit)),
        };
        self.update(|until| match until {
            Until::Carried | Until::Unsettled => done,
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
        let mut held = Vec::new();
        self.entries.retain_mut(|entry| {
            let gone = match entry.until {
                Until::Kept(None) => true,
                Until::Kept(Some(commit)) => reached.contains(&commit),
                _ => false,
            };
            let stays = !gone || entry.failed;
            if !stays {
                held.append(&mut entry.announcements);
            }
            stays
        });
        self.held.extend(held);
        self.changed |= self.entries.len() != before;
    }

    /// The stash this replica handed to an owner is kept in a commit of its own that only this
    /// device's outbox holds, until it drains.
    pub(crate) fn kept_here(&mut self, commit: CommitId) {
        let kept = self.keeping(commit);
        self.update(|until| {
            Some(match until {
                Until::Handed(handed) if handed == commit => kept,
                until => until,
            })
        });
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
        let mut held = Vec::new();
        self.entries.retain_mut(|entry| {
            if accepted > 0 && entry.until == Until::Unsettled {
                accepted -= 1;
                held.append(&mut entry.announcements);
                return false;
            }
            true
        });
        self.held.extend(held);
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
            // What was lost with the session is not in the content the next replica loads, so
            // nothing could show it: it goes with this replica.
            .filter(|entry| entry.until != Until::Dropped)
            .map(|entry| Entry {
                sequence: match entry.until {
                    Until::Sequenced(sequence) | Until::Lost(sequence) => Some(sequence),
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
