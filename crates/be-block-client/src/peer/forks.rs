//! Forks of a block's signed log across devices (guides/blocks/keys.md, "Forks"). Each device
//! keeps the head it verified and takes a later one only with a proof that it extends it, so a
//! server that cuts or rewrites a log is caught by every device that saw the head. One that
//! shows two devices two logs from the start is caught only when the devices compare heads:
//! a device told of a head another device kept asks the server to prove the two lie on one
//! log, and a pair it cannot prove is a fork, kept as a verdict.

use be_block_keys::merkle;
use be_block_protocol::{LogHead, LogPage};
use be_block_store::ObjectStore;
use uuid::Uuid;

use super::Peer;
use crate::{
    ClientError,
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
