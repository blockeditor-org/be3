//! Blocks' signed logs on a device (plans/signed_logs.md; guides/blocks/client.md, "Signed
//! logs"): entries made here wait in the outbox until the server takes them, and every page or
//! push the server serves is tied to the head it names by a Merkle proof, and that head to the
//! one this device kept by a consistency proof, so a page anywhere in a log costs that page and
//! O(log n) hashes, and a server that cut, rewrote or forked a log is caught.

use be_block_keys::merkle::{self, TreeHash};
use be_block_protocol::{ErrorCode, LogEntry, LogHead, LogPage, Logged, NewEntry, Op as Call};
use be_block_store::{Hash, ObjectStore};
use uuid::Uuid;

use super::{Drained, Peer, Signer};
use crate::{
    ClientError,
    refs::{Appending, KeptLogHead, Op},
};

/// Where an entry `Peer::append` made went.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Appended {
    /// The server took it: its place in the log and when it arrived.
    Logged { id: Uuid, seq: u64, arrival: i64 },
    /// It waits in the outbox, sent by the next drain: this device is offline, or something of
    /// the block (its creation, an earlier entry) waits ahead of it.
    Kept { id: Uuid },
}

/// An entry of a block's signed log as this device verified it: as the log holds it (its
/// payload sealed, or none once redacted), and its payload opened.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoggedEntry {
    pub entry: LogEntry,
    /// None once redacted, or when no key this device holds opens it.
    pub opened: Option<Vec<u8>>,
}

/// A run of a block's signed log, verified against `head`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogRead {
    pub head: LogHead,
    pub entries: Vec<LoggedEntry>,
    /// How many hashes tied the entries to the head, and the head to the one kept before:
    /// O(log n) wherever in the log they are, and none for what was read from this device.
    pub proof_hashes: usize,
}

/// How many times `read_log` reads a page again because the kept head moved while it was on
/// its way, before giving up as contended.
const READS_WHILE_MOVING: usize = 4;

/// The entries one drain sends of a block in one `append`.
const ENTRIES_PER_SEND: usize = 64;
/// The sealed bytes of entries one drain sends in one `append`, within the server's
/// `MAX_APPEND_BYTES`.
const BYTES_PER_SEND: usize = be_block_protocol::MAX_FRAME_BYTES / 16;

fn tree_hashes(hashes: &[Hash]) -> Vec<TreeHash> {
    hashes.iter().map(|hash| *hash.as_bytes()).collect()
}

impl<S: ObjectStore> Peer<S> {
    /// Appends `payload` to `block`'s signed log: sealed under the block's key, signed by this
    /// account, and kept in the outbox until the server takes it. Online with nothing of the
    /// block waiting ahead of it, it is sent at once, with any entries of the block that wait.
    pub async fn append(&self, block: Uuid, payload: &[u8]) -> Result<Appended, ClientError> {
        let key = self.sealing_key(block).await?;
        let sealed = self.commits.vault().seal(key, payload)?;
        if sealed.len() > be_block_protocol::MAX_LOG_ENTRY_BYTES {
            return Err(ClientError::Refused(
                ErrorCode::InvalidRequest,
                format!(
                    "a log entry may be at most {} bytes sealed, not {}",
                    be_block_protocol::MAX_LOG_ENTRY_BYTES,
                    sealed.len()
                ),
            ));
        }
        let id = Uuid::new_v4();
        let signature =
            self.sign_authored(block, &be_block_protocol::log_signed(id, Hash::of(&sealed)));
        self.refs.push(Op::Append(Appending {
            block,
            id,
            payload: sealed,
            signature,
        }))?;
        if self.local {
            self.settle_locally()?;
            let entry = self
                .refs
                .log_head(block)
                .map(|head| self.refs.log_entries(block, head.size.saturating_sub(1), 1))
                .unwrap_or_default();
            return match entry.first() {
                Some(entry) if entry.id == id => Ok(Appended::Logged {
                    id,
                    seq: entry.seq,
                    arrival: entry.arrival,
                }),
                _ => Ok(Appended::Kept { id }),
            };
        }
        if !self.is_attached() || self.refs.creating(block) {
            return Ok(Appended::Kept { id });
        }
        let drained = match self.send_appends(block).await {
            Ok(drained) => drained,
            Err(error) if error.is_unreachable() => return Ok(Appended::Kept { id }),
            Err(error) => return Err(error),
        };
        for outcome in drained {
            match outcome {
                Drained::Appended {
                    id: sent,
                    seq,
                    arrival,
                    ..
                } if sent == id => return Ok(Appended::Logged { id, seq, arrival }),
                Drained::NotAppended { id: sent, code, .. } if sent == id => {
                    return Err(ClientError::Refused(
                        code,
                        "the server refused the entry".into(),
                    ));
                }
                _ => {}
            }
        }
        Ok(Appended::Kept { id })
    }

    /// Sends the entries of `block` that wait in the outbox, in order, a batch per write: each
    /// one taken leaves the outbox, and one refused does too, told as `NotAppended`. Stops at
    /// the first failure to reach the server, leaving the rest.
    pub(super) async fn send_appends(&self, block: Uuid) -> Result<Vec<Drained>, ClientError> {
        let mut outcomes = Vec::new();
        loop {
            let mut batch = Vec::new();
            let mut bytes = 0;
            for (sequence, op) in self.refs.pending_for(block) {
                let Op::Append(appending) = op else {
                    continue;
                };
                if !batch.is_empty()
                    && (batch.len() >= ENTRIES_PER_SEND
                        || bytes + appending.payload.len() > BYTES_PER_SEND)
                {
                    break;
                }
                bytes += appending.payload.len();
                batch.push((sequence, appending));
            }
            if batch.is_empty() {
                return Ok(outcomes);
            }
            let entries = batch
                .iter()
                .map(|(_, appending)| NewEntry {
                    id: appending.id,
                    payload: appending.payload.clone(),
                    signature: appending.signature.clone(),
                })
                .collect();
            let sent = crate::ask(be_block_protocol::write::<Vec<Logged>, _>(
                &self.connection(),
                Some(self.organization),
                Call::Append { block, entries },
            ))
            .await;
            let sequences: Vec<u64> = batch.iter().map(|(sequence, _)| *sequence).collect();
            match sent {
                Ok(logged) if logged.len() == batch.len() => {
                    self.refs.remove(&sequences)?;
                    outcomes.extend(batch.iter().zip(logged).map(|((_, appending), logged)| {
                        Drained::Appended {
                            block,
                            id: appending.id,
                            seq: logged.seq,
                            arrival: logged.arrival,
                        }
                    }));
                }
                Ok(_) => return Err(ClientError::Unexpected),
                Err(ClientError::Refused(code, _)) => {
                    self.refs.remove(&sequences)?;
                    outcomes.extend(batch.iter().map(|(_, appending)| Drained::NotAppended {
                        block,
                        id: appending.id,
                        code,
                    }));
                }
                Err(error) if outcomes.is_empty() => return Err(error),
                Err(_) => return Ok(outcomes),
            }
        }
    }

    /// A local account's entry, numbered and stamped on this device as a server would, with the
    /// log's tree kept by its right edge (`KeptLogHead::frontier`).
    pub(super) fn append_locally(
        &self,
        sequence: u64,
        appending: Appending,
    ) -> Result<Drained, ClientError> {
        let block = appending.block;
        let kept = self.refs.log_head(block);
        let mut frontier = kept
            .as_ref()
            .and_then(|kept| kept.frontier.clone())
            .unwrap_or_default();
        let last = match frontier.size() {
            0 => None,
            size => self.refs.log_entries(block, size - 1, 1).pop(),
        };
        let arrival =
            be_block_commit::now_milliseconds().max(last.map_or(i64::MIN, |last| last.arrival));
        let entry = LogEntry {
            seq: frontier.size(),
            id: appending.id,
            author: self.account(),
            arrival,
            payload_hash: Hash::of(&appending.payload),
            signature: appending.signature,
            payload: Some(appending.payload),
        };
        frontier.push(entry.leaf());
        let (seq, id) = (entry.seq, entry.id);
        // Settling runs one at a time, so the head is still the one it was read as.
        self.refs.keep_entries(
            block,
            kept.as_ref(),
            KeptLogHead {
                size: frontier.size(),
                root: frontier.root(),
                frontier: Some(frontier),
            },
            vec![entry],
        )?;
        self.refs.remove(&[sequence])?;
        Ok(Drained::Appended {
            block,
            id,
            seq,
            arrival,
        })
    }

    /// Reads at most `limit` entries of `block`'s signed log from `from`, following the log so
    /// what is appended later arrives as `Event::Appended`. The server's page is taken only as
    /// its proof ties it to the head it names and that head extends the one this device kept;
    /// then the head and the entries are kept, and opened, the block's keys asked for once if
    /// one is missing. Offline, and for a local account, what this device kept, from `from` up
    /// to the first entry it does not hold.
    pub async fn read_log(
        &self,
        block: Uuid,
        from: u64,
        limit: u32,
    ) -> Result<LogRead, ClientError> {
        let kept = self.refs.log_head(block);
        if self.local || !self.is_attached() {
            let head = kept.map_or_else(
                || LogHead {
                    size: 0,
                    root: Hash::from_bytes(merkle::empty_root()),
                },
                |kept| LogHead {
                    size: kept.size,
                    root: Hash::from_bytes(kept.root),
                },
            );
            let entries = self.refs.log_entries(block, from, limit as usize);
            return Ok(LogRead {
                head,
                entries: entries
                    .into_iter()
                    .map(|entry| self.opened(block, entry))
                    .collect(),
                proof_hashes: 0,
            });
        }
        // Another read or a push may move the kept head while this page is on its way: a page
        // checked against the head that moved is read again against the one kept now.
        for _ in 0..READS_WHILE_MOVING {
            let kept = self.refs.log_head(block);
            let page = crate::ask(be_block_protocol::ask::<LogPage, _>(
                &self.connection(),
                Some(self.organization),
                be_block_protocol::Query::BlockLog {
                    block,
                    from,
                    limit,
                    known: kept.as_ref().map(|kept| kept.size),
                },
                true,
            ))
            .await?;
            let proof_hashes = page.proof.len() + page.consistency.len();
            // A page from past the log's end is answered from its end, empty.
            let from = from.min(page.head.size);
            let Some(entries) = self.take_entries(
                block,
                kept.as_ref(),
                from,
                page.head,
                page.entries,
                &page.proof,
                &page.consistency,
            )?
            else {
                continue;
            };
            let entries = self.open_entries(block, entries).await?;
            return Ok(LogRead {
                head: page.head,
                entries,
                proof_hashes,
            });
        }
        Err(ClientError::Contended)
    }

    /// Takes an append the server told of a log this device follows: at once when it extends
    /// the head this device kept, which it proves itself; otherwise, having missed some, by
    /// reading the log from the kept head. Answers the entries it took that this device did not
    /// have, in order.
    pub async fn logged(
        &self,
        appended: be_block_protocol::Appended,
    ) -> Result<Vec<LoggedEntry>, ClientError> {
        let block = appended.block;
        let kept = self.refs.log_head(block);
        let size = kept.as_ref().map_or(0, |kept| kept.size);
        if appended.head.size <= size {
            // Told late, of what this device read since: still, nothing it names may differ
            // from what this device holds.
            let forked = kept.as_ref().is_some_and(|kept| {
                appended.head.size == kept.size && *appended.head.root.as_bytes() != kept.root
            }) || appended.entries.iter().any(|entry| {
                self.refs
                    .log_entry(block, entry.seq)
                    .is_some_and(|held| held.leaf() != entry.leaf())
            });
            return match forked {
                true => Err(ClientError::LogForked(block)),
                false => Ok(Vec::new()),
            };
        }
        if appended.previous == size
            && let Some(entries) = self.take_entries(
                block,
                kept.as_ref(),
                appended.previous,
                appended.head,
                appended.entries,
                &appended.proof,
                &appended.consistency,
            )?
        {
            return self.open_entries(block, entries).await;
        }
        // It missed some, or the kept head moved meanwhile: read on from the kept head.
        let size = self.refs.log_head(block).map_or(0, |kept| kept.size);
        Ok(self.read_log(block, size, u32::MAX).await?.entries)
    }

    /// Verifies entries from `from` against `head` with `proof`, and `head` against `kept` with
    /// `consistency`, then keeps them and the head: a page that does not match the head it
    /// names, names an id twice, or whose arrivals go back (within it, or against the entries
    /// kept beside it) is tampered, and a head that does not extend the kept one is the server
    /// cutting or forking the log (`ClientError::LogForked`), and nothing of it is kept. None,
    /// keeping nothing, when the kept head moved from `kept` meanwhile (`Refs::keep_entries`).
    #[allow(clippy::too_many_arguments)]
    fn take_entries(
        &self,
        block: Uuid,
        kept: Option<&KeptLogHead>,
        from: u64,
        head: LogHead,
        entries: Vec<LogEntry>,
        proof: &[Hash],
        consistency: &[Hash],
    ) -> Result<Option<Vec<LogEntry>>, ClientError> {
        let root = *head.root.as_bytes();
        // The head first: a log cut or forked is that, whatever its entries say.
        if let Some(kept) = kept
            && !merkle::consistent(
                kept.size,
                &kept.root,
                head.size,
                &root,
                &tree_hashes(consistency),
            )
        {
            return Err(ClientError::LogForked(block));
        }
        let mut ids = std::collections::HashSet::with_capacity(entries.len());
        let distinct = entries.iter().all(|entry| ids.insert(entry.id));
        let before = from
            .checked_sub(1)
            .and_then(|seq| self.refs.log_entry(block, seq));
        let after = self.refs.log_entry(block, from + entries.len() as u64);
        let arrivals: Vec<i64> = before
            .iter()
            .chain(&entries)
            .chain(after.iter())
            .map(|entry| entry.arrival)
            .collect();
        let forward = arrivals.windows(2).all(|pair| pair[0] <= pair[1]);
        let in_order = entries
            .iter()
            .enumerate()
            .all(|(index, entry)| entry.seq == from + index as u64);
        let whole = entries.iter().all(|entry| {
            entry
                .payload
                .as_ref()
                .is_none_or(|payload| Hash::of(payload) == entry.payload_hash)
        });
        let leaves: Vec<TreeHash> = entries.iter().map(LogEntry::leaf).collect();
        if !in_order
            || !whole
            || !distinct
            || !forward
            || merkle::range_root(head.size, from, &leaves, &tree_hashes(proof)) != Some(root)
        {
            return Err(ClientError::Tampered(head.root));
        }
        // Consistent, the head is at least as long as the kept one.
        let kept_now = self.refs.keep_entries(
            block,
            kept,
            KeptLogHead {
                size: head.size,
                root,
                frontier: None,
            },
            entries.clone(),
        )?;
        Ok(kept_now.then_some(entries))
    }

    /// `entries` opened, the block's keys asked for once when one of them is sealed under a key
    /// this device does not hold yet (a rotation since it last asked).
    async fn open_entries(
        &self,
        block: Uuid,
        entries: Vec<LogEntry>,
    ) -> Result<Vec<LoggedEntry>, ClientError> {
        let missing = entries.iter().any(|entry| {
            entry.payload.as_ref().is_some_and(|sealed| {
                be_block_store::sealed_key(sealed).is_ok_and(|key| !self.keys.has(key))
            })
        });
        if missing && self.is_attached() {
            self.refresh_keys(Some(vec![block])).await?;
        }
        Ok(entries
            .into_iter()
            .map(|entry| self.opened(block, entry))
            .collect())
    }

    /// The head of `block`'s signed log this device last verified: every head the server
    /// serves later must extend it.
    pub fn log_head(&self, block: Uuid) -> Option<LogHead> {
        self.refs.log_head(block).map(|kept| LogHead {
            size: kept.size,
            root: Hash::from_bytes(kept.root),
        })
    }

    fn opened(&self, block: Uuid, entry: LogEntry) -> LoggedEntry {
        let opened = entry
            .payload
            .as_ref()
            .and_then(|sealed| self.open_for(block, sealed).ok());
        LoggedEntry { entry, opened }
    }

    /// Who signed an entry of `block`'s log (`verify_authored`), judged by when the server took
    /// it: its arrival, which no author steers, so one signed with a contact's replaced key that
    /// arrived after this device learned of the new key answers `Replaced`.
    pub async fn verify_entry(&self, block: Uuid, entry: &LogEntry) -> Result<Signer, ClientError> {
        self.verify_authored(
            entry.author,
            block,
            &entry.signed(),
            &entry.signature,
            Some(entry.arrival),
        )
        .await
    }

    /// Drops the payload of the entry at `seq` of `block`'s log on the server, for everyone: its
    /// author's, or a manager's. Needs the server.
    pub async fn redact(&self, block: Uuid, seq: u64) -> Result<(), ClientError> {
        if self.local {
            return Err(ClientError::Refused(
                ErrorCode::InvalidRequest,
                "a local account's log keeps what it holds".into(),
            ));
        }
        crate::ask(be_block_protocol::write::<(), _>(
            &self.connection(),
            Some(self.organization),
            Call::Redact { block, seq },
        ))
        .await
    }
}
