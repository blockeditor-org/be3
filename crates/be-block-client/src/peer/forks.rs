//! Forks of a block's signed log across devices (guides/blocks/keys.md, "Forks"). Each device
//! keeps the head it verified and takes a later one only with a proof that it extends it, so a
//! server that cuts or rewrites a log is caught by every device that saw the head. One that
//! shows two devices two logs from the start is caught only when the devices compare heads: a
//! device signs the head it kept with its account's key (`SignedLogHead`), so the server can
//! neither make one up nor change one, and a device told of such a head asks the server to
//! prove the two lie on one log. A pair it proves cannot is a fork, kept as a verdict.

use be_block_keys::{SigningPublic, merkle};
use be_block_protocol::{ClientId, LogHead, LogPage};
use be_block_store::{Hash, ObjectStore};
use uuid::Uuid;

use super::Peer;
use crate::{
    Answer, ClientError,
    refs::{HeadFrom, KeptLogHead, LogFork},
};

/// The head of a block's signed log a device of `account` kept, signed with the account's key
/// (`RecoverySecret::sign_log_head`): what devices compare (`Peer::compare_log_head`). The
/// account's devices share the key, so the signature names the account, not the device.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedLogHead {
    pub account: Uuid,
    pub head: LogHead,
    pub signature: Vec<u8>,
}

/// How a signed head reached this device (`Peer::compare_log_head`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeadVia {
    /// In the block's live session (`Peer::tell_log_head`).
    Session,
    /// In a commit of the block (`Commit::log_through`).
    Commit,
}

/// How a head of a block's signed log another device kept stands against this device's.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LogAgreement {
    /// Both lie on one log: the same head, or the server proved the shorter a prefix of the
    /// longer.
    Agrees,
    /// They do not: the server showed the two devices different logs. The verdict this device
    /// keeps for the block, the first it found (`Peer::log_fork`).
    Forked(LogFork),
}

/// The first byte of a relay a block's type module sent (`Peer::relay`).
pub(crate) const MODULE_RELAY: u8 = 0;
/// The first byte of a relay naming the head of the block's signed log the sender kept,
/// signed (`Peer::tell_log_head`): its account, the head's size and root, the signature.
const LOG_HEAD_RELAY: u8 = 1;
const SIGNATURE_BYTES: usize = 64;
const LOG_HEAD_BYTES: usize = 1 + 16 + 8 + 32 + SIGNATURE_BYTES;

/// What a participant of a block's live session relayed, unsealed (`Peer::unseal`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Relayed {
    /// A message of the block's type module, which the stack hands it.
    Module(Vec<u8>),
    /// The head of the block's signed log a device kept, which this device compares with its
    /// own (`Peer::compare_log_head`), checking its signature first.
    LogHead(SignedLogHead),
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
                let root: Result<[u8; 32], _> = plain[25..57].try_into();
                match (account, size, root) {
                    (Ok(account), Ok(size), Ok(root)) => Self::LogHead(SignedLogHead {
                        account,
                        head: LogHead {
                            size,
                            root: Hash::from_bytes(root),
                        },
                        signature: plain[57..].to_vec(),
                    }),
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
    /// The head of `block`'s signed log this device kept, signed with the account's key, for
    /// another device to compare: none for a log this device never read.
    pub fn signed_log_head(&self, block: Uuid) -> Option<SignedLogHead> {
        let kept = self.refs.log_head(block)?;
        Some(SignedLogHead {
            account: self.account(),
            head: LogHead {
                size: kept.size,
                root: Hash::from_bytes(kept.root),
            },
            signature: self
                .keys
                .identity()
                .sign_log_head(block.into_bytes(), kept.size, &kept.root),
        })
    }

    /// Compares `theirs`, a head of `block`'s signed log another device kept, with the head
    /// this device kept (guides/blocks/keys.md, "Forks").
    ///
    /// It takes nothing on the server's word: `theirs` must be signed with a key this device
    /// verified for its account (this account's own, or a contact's), else it is
    /// `ClientError::Forged` and nothing is kept, so a server cannot plant a fork. A head of
    /// size 0 whose root is not the empty tree's is `Tampered`. Then this device reads the
    /// log's head as the server serves it now, proved to extend the one it kept (`read_log`):
    /// a head of the same size must be that one; for a shorter one the server answers its root
    /// at that size with a proof that the head just read extends it (`log-page.known-root`),
    /// and a proof that does not check is `Tampered`, the server misbehaving, not a fork. It is
    /// a fork when that proved root is not theirs (kept as evidence, `LogFork::prefix`), and
    /// when theirs is longer: this assumes one linearizable server per log, which only grows,
    /// so this device, reading after the other kept its head, holds a head at least as long.
    /// A fork is kept, the first per block (`Refs::found_fork`), and told once
    /// (`Event::LogForked`); one found later answers that one. Needs the server: offline, or
    /// for a local account, `ClientError::Disconnected`.
    pub async fn compare_log_head(
        &self,
        block: Uuid,
        theirs: SignedLogHead,
        via: HeadVia,
    ) -> Result<LogAgreement, ClientError> {
        if !self.vouched(block, &theirs) {
            return Err(ClientError::Forged(block));
        }
        if theirs.head.size == 0 && *theirs.head.root.as_bytes() != merkle::empty_root() {
            return Err(ClientError::Tampered(theirs.head.root));
        }
        if self.local || !self.is_attached() {
            return Err(ClientError::Disconnected(
                "comparing log heads needs the server".to_owned(),
            ));
        }
        let from = match via {
            HeadVia::Session => HeadFrom::Session {
                account: theirs.account,
                signature: theirs.signature.clone(),
            },
            HeadVia::Commit => HeadFrom::Commit {
                author: theirs.account,
                signature: theirs.signature.clone(),
            },
        };
        let head = theirs.head;
        for _ in 0..COMPARES_WHILE_GROWING {
            let ours = self.read_log(block, u64::MAX, 1).await?.head;
            if head.size >= ours.size {
                return match head == ours {
                    true => Ok(LogAgreement::Agrees),
                    false => self.forked(block, &ours, &head, None, from),
                };
            }
            // Their size's root as the server holds it, proved a prefix of the head the server
            // serves now, which must still be the one just read: one that grew meanwhile is
            // read again.
            let page = crate::ask(be_block_protocol::ask::<LogPage, _>(
                &self.connection(),
                Some(self.organization),
                be_block_protocol::Query::BlockLog {
                    block,
                    from: u64::MAX,
                    limit: 1,
                    known: Some(head.size),
                    redactions_after: None,
                },
                true,
            ))
            .await?;
            if page.head.size > ours.size {
                continue;
            }
            if page.head != ours {
                // The server serves this device a head that does not extend the one just read.
                return Err(self.own_fork(block, &ours, &page.head));
            }
            let proof: Vec<merkle::TreeHash> =
                page.consistency.iter().map(|hash| *hash.as_bytes()).collect();
            let Some(prefix) = page.known_root.filter(|prefix| {
                merkle::consistent(
                    head.size,
                    prefix.as_bytes(),
                    ours.size,
                    ours.root.as_bytes(),
                    &proof,
                )
            }) else {
                return Err(ClientError::Tampered(page.head.root));
            };
            return match prefix == head.root {
                true => Ok(LogAgreement::Agrees),
                false => self.forked(block, &ours, &head, Some(*prefix.as_bytes()), from),
            };
        }
        Err(ClientError::Contended)
    }

    /// Tells a participant of `block`'s live session (`to`), or all of them, the head of its
    /// signed log this device kept, signed (`signed_log_head`), so each compares it with its
    /// own: as this device joins the session, and to each that joins after it. Nothing for a
    /// log this device never read, or a local account.
    pub async fn tell_log_head(
        &self,
        block: Uuid,
        to: Option<ClientId>,
    ) -> Result<Option<Answer<()>>, ClientError> {
        let Some(signed) = self.signed_log_head(block).filter(|_| !self.local) else {
            return Ok(None);
        };
        let mut framed = Vec::with_capacity(LOG_HEAD_BYTES);
        framed.push(LOG_HEAD_RELAY);
        framed.extend_from_slice(signed.account.as_bytes());
        framed.extend_from_slice(&signed.head.size.to_le_bytes());
        framed.extend_from_slice(signed.head.root.as_bytes());
        framed.extend_from_slice(&signed.signature);
        Ok(Some(self.relay_framed(block, to, &framed).await?))
    }

    /// Compares the head of `block`'s signed log `commit` names (`Commit::log_through`), signed
    /// by its author, with the one this device kept (`compare_log_head`): each such head once a
    /// run, and only for a log this device reads and a head that is not the one it kept, so a
    /// commit made on the log as this device sees it asks the server nothing. What goes wrong
    /// asking is let go: it is no reason to refuse the commit.
    pub(crate) async fn compare_commit_head(&self, block: Uuid, commit: &be_block_commit::Commit) {
        let Some(named) = commit.log_through.as_ref() else {
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
        let theirs = SignedLogHead {
            account: commit.author,
            head: LogHead {
                size: named.size,
                root: Hash::from_bytes(named.root),
            },
            signature: named.signature.clone(),
        };
        if self
            .compare_log_head(block, theirs, HeadVia::Commit)
            .await
            .is_err()
        {
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

    /// Whether `signed` is signed with a key this device verified for its account: this
    /// account's own, or a key a contact's card or key change vouched for. A key only the
    /// server names vouches for nothing, since the server could name its own.
    fn vouched(&self, block: Uuid, signed: &SignedLogHead) -> bool {
        let verifies = |key: SigningPublic| {
            key.verifies_log_head(
                block.into_bytes(),
                signed.head.size,
                signed.head.root.as_bytes(),
                &signed.signature,
            )
        };
        if signed.account == self.account() {
            return verifies(self.keys.identity().signing());
        }
        self.refs
            .signers(signed.account)
            .iter()
            .any(|known| known.verified && verifies(SigningPublic(known.key)))
    }

    /// The server served this device `served`, which does not extend `kept`: kept as the
    /// block's fork (`HeadFrom::Server`), so one place answers whether it forked, and answered
    /// as `ClientError::LogForked`.
    pub(crate) fn own_fork(&self, block: Uuid, kept: &LogHead, served: &LogHead) -> ClientError {
        if let Err(error) = self.forked(block, kept, served, None, HeadFrom::Server) {
            return error;
        }
        ClientError::LogForked(block)
    }

    fn forked(
        &self,
        block: Uuid,
        ours: &LogHead,
        theirs: &LogHead,
        prefix: Option<[u8; 32]>,
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
            prefix,
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
