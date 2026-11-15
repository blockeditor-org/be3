//! The journal of a block's session (guides/blocks/client.md, "The journal"): what its owner
//! sequenced since the last commit, sealed with the block's key, kept on the server and in this
//! device's refs database.

use be_block_content::Announcement;
use be_block_protocol::{Journal, JournalBase, JournalEntry};
use be_block_store::{Hash, ObjectStore};
use uuid::Uuid;

use super::{Peer, locked};
use crate::{ClientError, connection::Answer, refs::Until};

impl<S: ObjectStore> Peer<S> {
    /// Seals `entries` (plain payloads), keeps them in this device's journal and, when
    /// `online`, appends them to the server's, which with `relay` also relays each to the
    /// session's other participants: one call both broadcasts and appends. `announcements`
    /// (by the sequence of the edit) are kept beside the entries until the server holds them
    /// (`journal_held`, or the commit `rebase_journal` hands them to).
    #[allow(clippy::too_many_arguments)]
    pub async fn append(
        &self,
        block: Uuid,
        generation: u64,
        tag: Hash,
        entries: Vec<JournalEntry>,
        relay: bool,
        online: bool,
        announcements: Vec<(u64, Announcement)>,
    ) -> Result<Option<Answer<()>>, ClientError> {
        let key = self.sealing_key(block).await?;
        let sealed = entries
            .into_iter()
            .map(|entry| -> Result<JournalEntry, ClientError> {
                Ok(JournalEntry {
                    payload: self.commits.vault().seal(key, &entry.payload)?,
                    ..entry
                })
            })
            .collect::<Result<Vec<_>, ClientError>>()?;
        self.refs
            .append_journal(block, tag, generation, sealed.clone())?;
        for (sequence, announcement) in announcements {
            let made = self.made(key, announcement)?;
            self.refs
                .add_announcements(block, Until::Journal(sequence), vec![made])?;
        }
        Ok(online.then(|| {
            self.connection()
                .append(block, generation, tag, sealed, relay)
        }))
    }

    /// Seals `base`'s state and starts this device's journal, and when `online` the server's,
    /// from it. What the journal's entries announce goes with `base`'s commit when this device
    /// sealed it (`own`), sent once the server holds it; the rest is dropped.
    /// Starts this device's journal, and when `online` the server's, from `base`, keeping
    /// `entries` (plain payloads) past it. One write holds on the device only, so no crash
    /// there loses them; the server gets a rebase and then an append, and a drop between the
    /// two is covered by the device's journal, which a replica opens on when it holds more.
    #[allow(clippy::too_many_arguments)]
    pub async fn rebase_journal(
        &self,
        block: Uuid,
        generation: u64,
        tag: Hash,
        base: JournalBase,
        entries: Vec<JournalEntry>,
        online: bool,
        own: bool,
    ) -> Result<Option<Answer<()>>, ClientError> {
        let key = self.sealing_key(block).await?;
        let sealed = JournalBase {
            state: self.commits.vault().seal(key, &base.state)?,
            ..base
        };
        let entries = entries
            .into_iter()
            .map(|entry| -> Result<JournalEntry, ClientError> {
                Ok(JournalEntry {
                    payload: self.commits.vault().seal(key, &entry.payload)?,
                    ..entry
                })
            })
            .collect::<Result<Vec<_>, ClientError>>()?;
        let (commit, sequence) = (sealed.commit, sealed.sequence);
        let carried = entries.iter().map(|entry| entry.sequence).max();
        self.refs
            .rebase_journal(block, tag, generation, sealed.clone(), entries.clone())?;
        let kept = commit.is_some_and(|commit| self.pending_commits(block).contains(&commit));
        self.refs
            .journal_rebased(block, commit, sequence, carried, own, kept)?;
        Ok(online.then(|| {
            let connection = self.connection();
            let rebased = connection.rebase_journal(block, generation, tag, sealed);
            let appended = (!entries.is_empty())
                .then(|| connection.append(block, generation, tag, entries, false));
            Box::pin(async move {
                rebased.await?;
                match appended {
                    Some(appended) => appended.await,
                    None => Ok(()),
                }
            }) as Answer<()>
        }))
    }

    /// The server took the journal entries holding the announcements `keys` name.
    pub fn journal_held(
        &self,
        block: Uuid,
        keys: &std::collections::HashSet<String>,
    ) -> Result<(), ClientError> {
        Ok(self.refs.journal_held(block, keys)?)
    }

    /// Makes this device's journal of `block` the server's `journal`, sealed as joining answered
    /// it: a replica opened on it journals what it sequences next after the server's entries,
    /// and this device keeps what it opened on.
    pub fn keep_journal(&self, block: Uuid, journal: &Journal) -> Result<(), ClientError> {
        let generation = journal
            .entries
            .iter()
            .map(|entry| entry.generation)
            .max()
            .unwrap_or_default();
        self.refs.rebase_journal(
            block,
            journal.tag,
            generation,
            journal.base.clone(),
            journal.entries.clone(),
        )?;
        Ok(())
    }

    /// This device's journal of `block`, sealed as it keeps it.
    pub fn sealed_journal(&self, block: Uuid) -> Option<Journal> {
        self.refs.journal(block)
    }

    /// This device's journal of `block`, opened.
    pub fn local_journal(&self, block: Uuid) -> Result<Option<Journal>, ClientError> {
        self.refs
            .journal(block)
            .map(|journal| self.open_journal(journal))
            .transpose()
    }

    /// Opens a sealed journal: its base state and each entry's payload.
    pub fn open_journal(&self, journal: Journal) -> Result<Journal, ClientError> {
        let vault = self.commits.vault();
        let open = |sealed: &[u8]| -> Result<Vec<u8>, ClientError> {
            match sealed.is_empty() {
                true => Ok(Vec::new()),
                false => vault.open(sealed).map_err(locked),
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
