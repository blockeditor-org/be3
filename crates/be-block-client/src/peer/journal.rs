//! The journal of a block's session (guides/blocks/client.md, "The journal"): the session's
//! order since its last seal, sealed with the block's key, kept on the server and in this
//! device's refs database.

use be_block_content::Announcement;
use be_block_protocol::{Journal, JournalBase, JournalEntry};
use be_block_session::Submission;
use be_block_store::{Hash, ObjectStore};
use uuid::Uuid;

use super::{Peer, locked};
use crate::{ClientError, connection::Answer, refs::Until};

/// Where a submission went: to the server's order, answered once it numbered it, or numbered
/// at once in this device's own order while offline.
pub enum Submitted {
    Server(Answer<()>),
    Offline(JournalEntry),
}

impl<S: ObjectStore> Peer<S> {
    /// Seals `submission`'s payload and hands it to the block's order: the server's when
    /// `online`, which every participant hears as `Event::Sequenced`, otherwise this device's
    /// own (`Refs::number_offline`), whose entry the caller delivers to the replica.
    /// `announcements` go once the server holds the edit: at once when it answers, or offline
    /// beside the entry until a commit of this device holds it (`rebase_journal`).
    pub async fn submit(
        &self,
        block: Uuid,
        tag: Hash,
        submission: Submission,
        online: bool,
        announcements: Vec<Announcement>,
    ) -> Result<Submitted, ClientError> {
        let key = self.sealing_key(block).await?;
        let submission = Submission {
            payload: self.commits.vault().seal(key, &submission.payload)?,
            ..submission
        };
        let made = announcements
            .into_iter()
            .map(|announcement| self.made(key, announcement))
            .collect::<Result<Vec<_>, _>>()?;
        if !online {
            let entry = self.refs.number_offline(block, tag, submission)?;
            self.refs
                .add_announcements(block, Until::Journal(entry.sequence), made)?;
            return Ok(Submitted::Offline(self.open_entry(block, entry)?));
        }
        self.refs.add_announcements(block, Until::Submitted, made)?;
        Ok(Submitted::Server(
            self.connection().submit(block, submission),
        ))
    }

    /// The server numbered the submissions holding the announcements `keys` name.
    pub fn submitted_held(
        &self,
        block: Uuid,
        keys: &std::collections::HashSet<String>,
    ) -> Result<(), ClientError> {
        Ok(self.refs.submitted_held(block, keys)?)
    }

    /// Keeps an entry the server's order numbered (sealed, as it arrived) in this device's
    /// journal, so a replica opened offline later opens on what this device heard.
    pub fn heard_entry(
        &self,
        block: Uuid,
        tag: Hash,
        entry: JournalEntry,
    ) -> Result<(), ClientError> {
        Ok(self.refs.append_journal(block, tag, vec![entry])?)
    }

    /// Seals `base`'s state and starts this device's journal, and when `online` the server's,
    /// from it, keeping what came after it. What the journal's entries
    /// up to the base announce goes with `base`'s commit when this device sealed it (`own`),
    /// sent once the server holds it; the rest is dropped.
    #[allow(clippy::too_many_arguments)]
    pub async fn rebase_journal(
        &self,
        block: Uuid,
        generation: u64,
        tag: Hash,
        base: JournalBase,
        online: bool,
        own: bool,
    ) -> Result<Option<Answer<()>>, ClientError> {
        let key = self.sealing_key(block).await?;
        let sealed = JournalBase {
            state: self.commits.vault().seal(key, &base.state)?,
            ..base
        };
        let (commit, sequence) = (sealed.commit, sealed.sequence);
        self.refs.rebase_journal(block, tag, sealed.clone())?;
        let kept = commit.is_some_and(|commit| self.pending_commits(block).contains(&commit));
        self.refs
            .journal_rebased(block, commit, sequence, own, kept)?;
        Ok(online.then(|| self.connection().rebase_journal(block, generation, sealed)))
    }

    /// Makes this device's journal of `block` the server's `journal`, sealed as joining answered
    /// it: this device keeps what it opened on, and adds what it hears numbered after.
    pub fn keep_journal(&self, block: Uuid, journal: &Journal) -> Result<(), ClientError> {
        self.refs.replace_journal(block, journal.clone())?;
        Ok(())
    }

    /// Keeps what the host holds of an open block's local edits no journal holds yet, each
    /// sealed with the block's key; none forgets them (`refs::OpenEdits`).
    pub async fn keep_open_edits(
        &self,
        block: Uuid,
        kept: crate::OpenEdits,
        edits: Vec<crate::KeptEdit>,
    ) -> Result<(), ClientError> {
        // Refused before anything is sealed, which may ask the server for the block's key.
        if self.refs.discarded(block) {
            return Err(ClientError::Discarded(block));
        }
        let mut sealed = Vec::with_capacity(edits.len());
        if !edits.is_empty() {
            let key = self.sealing_key(block).await?;
            for edit in &edits {
                let encoded = be_block_store::encode_value(edit)?;
                sealed.push(self.commits.vault().seal(key, &encoded)?);
            }
        }
        let kept = self.refs.keep_open_edits(
            block,
            crate::OpenEdits {
                edits: sealed,
                ..kept
            },
        )?;
        match kept {
            true => Ok(()),
            false => Err(ClientError::Discarded(block)),
        }
    }

    /// What the host kept of `block`'s replica before it last stopped: its origin and counts,
    /// and its local edits, opened. The edits fail to open while their key is not on this
    /// device (`ClientError::Locked`), and stay kept as they are.
    #[allow(clippy::type_complexity)]
    pub fn open_edits(
        &self,
        block: Uuid,
    ) -> Option<(crate::OpenEdits, Result<Vec<crate::KeptEdit>, ClientError>)> {
        let kept = self.refs.open_edits(block)?;
        let edits = kept
            .edits
            .iter()
            .map(|sealed| {
                let plain = self.open_for(block, sealed).map_err(locked)?;
                Ok(be_block_store::decode_value(&plain)?)
            })
            .collect();
        Some((kept, edits))
    }

    /// Moves `block`'s kept edits aside, unread and never sent, so a replica opens it under a
    /// fresh origin (`refs::SetAside`).
    pub fn set_aside_open_edits(&self, block: Uuid) -> Result<(), ClientError> {
        Ok(self.refs.set_aside(block)?)
    }

    /// Kept edits set aside that read now: each block's count of them, left as tombstones, to
    /// be told they read and were not sent.
    pub fn read_set_aside(&self) -> Vec<(Uuid, usize)> {
        let mut recovered = Vec::new();
        for (block, aside) in self.refs.set_aside_edits() {
            for entry in &aside {
                let Some(kept) = &entry.kept else {
                    continue;
                };
                let opened: Result<Vec<crate::KeptEdit>, ClientError> = kept
                    .edits
                    .iter()
                    .map(|sealed| {
                        let plain = self.open_for(block, sealed).map_err(locked)?;
                        Ok(be_block_store::decode_value(&plain)?)
                    })
                    .collect();
                if opened.is_ok() && self.refs.tombstone_set_aside(block, entry.id).is_ok() {
                    recovered.push((block, entry.count));
                }
            }
        }
        recovered
    }

    /// Keeps `kept` as `block`'s, its edits sealed already, as another device kept them.
    #[cfg(test)]
    pub(crate) fn keep_sealed_open_edits(&self, block: Uuid, kept: crate::OpenEdits) {
        assert!(self.refs.keep_open_edits(block, kept).unwrap());
    }

    /// What this device keeps of `block`'s edits set aside.
    #[cfg(test)]
    pub(crate) fn set_aside_of(&self, block: Uuid) -> Vec<crate::SetAside> {
        self.refs
            .set_aside_edits()
            .into_iter()
            .find(|(of, _)| *of == block)
            .map(|(_, aside)| aside)
            .unwrap_or_default()
    }

    /// What this device keeps of `block`'s replica, its edits sealed.
    #[cfg(test)]
    pub(crate) fn sealed_open_edits(&self, block: Uuid) -> Option<crate::OpenEdits> {
        self.refs.open_edits(block)
    }

    /// This device's journal of `block`, sealed as it keeps it.
    pub fn sealed_journal(&self, block: Uuid) -> Option<Journal> {
        self.refs.journal(block)
    }

    /// This device's journal of `block`, opened.
    pub fn local_journal(&self, block: Uuid) -> Result<Option<Journal>, ClientError> {
        self.refs
            .journal(block)
            .map(|journal| self.open_journal(block, journal))
            .transpose()
    }

    /// Opens the payload of an entry of `block`, sealed under one of its keys (`open_for`).
    pub fn open_entry(&self, block: Uuid, entry: JournalEntry) -> Result<JournalEntry, ClientError> {
        Ok(JournalEntry {
            payload: self.open_for(block, &entry.payload).map_err(locked)?,
            ..entry
        })
    }

    /// Opens `block`'s sealed journal: its base state and each entry's payload, each sealed
    /// under one of `block`'s keys (`Peer::open_for`).
    pub fn open_journal(&self, block: Uuid, journal: Journal) -> Result<Journal, ClientError> {
        let open = |sealed: &[u8]| -> Result<Vec<u8>, ClientError> {
            match sealed.is_empty() {
                true => Ok(Vec::new()),
                false => self.open_for(block, sealed).map_err(locked),
            }
        };
        Ok(Journal {
            tag: journal.tag,
            lost: journal.lost,
            base: JournalBase {
                state: open(&journal.base.state)?,
                ..journal.base
            },
            entries: journal
                .entries
                .into_iter()
                .map(|entry| -> Result<JournalEntry, ClientError> {
                    Ok(JournalEntry {
                        payload: open(&entry.payload)?,
                        ..entry
                    })
                })
                .collect::<Result<_, ClientError>>()?,
        })
    }
}
