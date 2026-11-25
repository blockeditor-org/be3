//! Forks of a block's signed log across devices (guides/blocks/keys.md, "Forks"). Each device
//! keeps the head it verified and takes a later one only with a proof that it extends it, so a
//! server that cuts or rewrites a log is caught by every device that saw the head. One that
//! shows two devices two logs from the start is caught only when the devices compare heads:
//! a device told of a head another device kept asks the server to prove the two lie on one
//! log, and a pair it cannot prove is a fork, kept as a verdict.

use be_block_keys::merkle;
use be_block_protocol::{ClientId, LogHead, LogPage};
use be_block_store::ObjectStore;
use uuid::Uuid;

use super::Peer;
use crate::{
    Answer, ClientError,
    refs::{HeadFrom, KeptLogHead, LogFork},
};

/// How a head of a block's signed log another device kept stands against this device's.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LogAgreement {
    /// Both lie on one log: the server proved the shorter a prefix of the longer.
    Agrees,
    /// They do not: the server showed the two devices different logs. The verdict this device
    /// keeps for the block, the first it found (`Peer::log_fork`).
    Forked(LogFork),
}

/// The first byte of a relay a block's type module sent (`Peer::relay`).
pub(crate) const MODULE_RELAY: u8 = 0;
/// The first byte of a relay naming the head of the block's signed log the sender kept
/// (`Peer::tell_log_head`): its account, then the head's size and root.
const LOG_HEAD_RELAY: u8 = 1;
const LOG_HEAD_BYTES: usize = 1 + 16 + 8 + 32;

/// What a participant of a block's live session relayed, unsealed (`Peer::unseal`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Relayed {
    /// A message of the block's type module, which the stack hands it.
    Module(Vec<u8>),
    /// The head of the block's signed log a device of `account` kept, which this device compares
    /// with its own (`Peer::compare_log_head`).
    LogHead { account: Uuid, head: LogHead },
    /// Bytes framed as neither: dropped.
    Unreadable,
}

impl Relayed {
    pub(crate) fn of(mut plain: Vec<u8>) -> Self {
        match plain.first() {
            Some(&MODULE_RELAY) => {
                plain.remove(0);
                Self::Module(plain)
            }
            Some(&LOG_HEAD_RELAY) if plain.len() == LOG_HEAD_BYTES => {
                let account = Uuid::from_slice(&plain[1..17]);
                let size = plain[17..25].try_into().map(u64::from_le_bytes);
                let root: Result<[u8; 32], _> = plain[25..].try_into();
                match (account, size, root) {
                    (Ok(account), Ok(size), Ok(root)) => Self::LogHead {
                        account,
                        head: LogHead {
                            size,
                            root: be_block_store::Hash::from_bytes(root),
                        },
                    },
                    _ => Self::Unreadable,
                }
            }
            _ => Self::Unreadable,
        }
    }
}

/// How many times a comparison reads the log again as it grows meanwhile.
const COMPARES_WHILE_GROWING: usize = 4;

impl<S: ObjectStore> Peer<S> {
    /// Compares `theirs`, a head of `block`'s signed log another device kept (told by `from`),
    /// with the head this device kept. This device first reads the log's head as the server
    /// serves it now, proved to extend the one it kept (`read_log`), so it holds the longest it
    /// may; then a head of the same size must be that one, and a shorter one must be proved a
    /// prefix of it by the server (a consistency proof). A longer one cannot lie on the log the
    /// server serves this device: the server's logs only grow, and this device read after the
    /// other device kept its head. A pair that does not prove is a fork: kept (`Refs::found_fork`)
    /// and told once (`Event::LogForked`). Needs the server: offline, or for a local account,
    /// `ClientError::Disconnected`.
    pub async fn compare_log_head(
        &self,
        block: Uuid,
        theirs: LogHead,
        from: HeadFrom,
    ) -> Result<LogAgreement, ClientError> {
        if self.local || !self.is_attached() {
            return Err(ClientError::Disconnected(
                "comparing log heads needs the server".to_owned(),
            ));
        }
        for _ in 0..COMPARES_WHILE_GROWING {
            let ours = self.read_log(block, u64::MAX, 1).await?.head;
            if theirs.size >= ours.size {
                return match theirs.size == ours.size && theirs.root == ours.root {
                    true => Ok(LogAgreement::Agrees),
                    false => self.forked(block, &ours, &theirs, from),
                };
            }
            // A proof from their size to the head the server serves now, which must still be
            // the one just read: one that grew meanwhile is read again.
            let page = crate::ask(be_block_protocol::ask::<LogPage, _>(
                &self.connection(),
                Some(self.organization),
                be_block_protocol::Query::BlockLog {
                    block,
                    from: u64::MAX,
                    limit: 1,
                    known: Some(theirs.size),
                    redactions_after: None,
                },
                true,
            ))
            .await?;
            if page.head.size != ours.size {
                continue;
            }
            let proof: Vec<merkle::TreeHash> =
                page.consistency.iter().map(|hash| *hash.as_bytes()).collect();
            let proved = page.head.root == ours.root
                && merkle::consistent(
                    theirs.size,
                    theirs.root.as_bytes(),
                    ours.size,
                    ours.root.as_bytes(),
                    &proof,
                );
            return match proved {
                true => Ok(LogAgreement::Agrees),
                false => self.forked(block, &ours, &theirs, from),
            };
        }
        Err(ClientError::Contended)
    }

    /// Tells a participant of `block`'s live session (`to`), or all of them, the head of its
    /// signed log this device kept, so each compares it with its own
    /// (guides/blocks/keys.md, "Forks"): as this device joins the session, and to each that
    /// joins after it. Nothing for a log this device never read, or a local account.
    pub async fn tell_log_head(
        &self,
        block: Uuid,
        to: Option<ClientId>,
    ) -> Result<Option<Answer<()>>, ClientError> {
        let Some(kept) = self.refs.log_head(block).filter(|_| !self.local) else {
            return Ok(None);
        };
        let mut framed = Vec::with_capacity(LOG_HEAD_BYTES);
        framed.push(LOG_HEAD_RELAY);
        framed.extend_from_slice(self.account().as_bytes());
        framed.extend_from_slice(&kept.size.to_le_bytes());
        framed.extend_from_slice(&kept.root);
        Ok(Some(self.relay_framed(block, to, &framed).await?))
    }

    /// Compares the head of `block`'s signed log `commit` names (`Commit::log_through`), made on
    /// another device, with the one this device kept (`compare_log_head`): each such head once a
    /// run, and only for a log this device reads and a head that is not the one it kept, so a
    /// commit made on the log as this device sees it asks the server nothing. What goes wrong
    /// asking is let go: it is no reason to refuse the commit.
    pub(crate) async fn compare_commit_head(&self, block: Uuid, commit: &be_block_commit::Commit) {
        let Some(named) = commit.log_through else {
            return;
        };
        let Some(kept) = self.refs.log_head(block).filter(|_| !self.local) else {
            return;
        };
        if (kept.size, kept.root) == (named.size, named.root)
            || !self
                .compared_heads
                .lock()
                .unwrap()
                .insert((block, named.size, named.root))
        {
            return;
        }
        let theirs = LogHead {
            size: named.size,
            root: be_block_store::Hash::from_bytes(named.root),
        };
        let from = HeadFrom::Commit {
            author: commit.author,
        };
        if self.compare_log_head(block, theirs, from).await.is_err() {
            // Asked again the next time it is loaded.
            self.compared_heads
                .lock()
                .unwrap()
                .remove(&(block, named.size, named.root));
        }
    }

    /// The fork of `block`'s signed log this device found first, if any.
    pub fn log_fork(&self, block: Uuid) -> Option<LogFork> {
        self.refs.log_fork(block)
    }

    fn forked(
        &self,
        block: Uuid,
        ours: &LogHead,
        theirs: &LogHead,
        from: HeadFrom,
    ) -> Result<LogAgreement, ClientError> {
        let kept = |head: &LogHead| KeptLogHead {
            size: head.size,
            root: *head.root.as_bytes(),
            frontier: None,
        };
        let fork = LogFork {
            block,
            ours: kept(ours),
            theirs: kept(theirs),
            from,
            found: self.server_now(),
        };
        if self.refs.found_fork(fork.clone())? {
            self.connection().fork_found(fork.clone());
            return Ok(LogAgreement::Forked(fork));
        }
        Ok(LogAgreement::Forked(
            self.refs.log_fork(block).unwrap_or(fork),
        ))
    }
}
