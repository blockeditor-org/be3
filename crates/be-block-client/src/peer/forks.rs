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
    refs::{ForkKind, HeadFrom, KeptLogHead, LogFork},
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
    /// In the account's block, from another device of this account
    /// (`Peer::trade_account_heads`).
    Account,
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

/// How many heads a device shares in the account's block a pass (`Peer::trade_account_heads`).
const SHARED_HEADS: usize = 64;

/// A head of a block's signed log as the account's block keeps it (`AccountBlock::log_heads`):
/// the organization the block is in, the head and the account's signature over it.
#[derive(Clone, Debug, serde::Deserialize, Eq, PartialEq, serde::Serialize)]
struct SharedHead {
    organization: Uuid,
    size: u64,
    root: [u8; 32],
    signature: Vec<u8>,
}

impl SharedHead {
    fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("a shared head always encodes")
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        postcard::from_bytes(bytes).ok()
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
            signature: self.keys.identity().sign_log_head(
                block.into_bytes(),
                kept.size,
                &kept.root,
            ),
        })
    }

    /// Compares `theirs`, a head of `block`'s signed log another device kept, with the head
    /// this device kept (guides/blocks/keys.md, "Forks").
    ///
    /// It takes nothing on the server's word: `theirs` must be signed with the key this device
    /// holds as current for its account (this account's own, or a contact's newest verified key
    /// that no broken chain superseded), else it is `ClientError::Forged` and nothing is kept,
    /// so a server cannot plant a fork. A device that has not followed its own account's new
    /// phrase so finds the heads its other devices sign with the new key `Forged`, which is
    /// intended: it cannot tell them from the server's. A head of size 0 whose root is not the
    /// empty tree's is `Tampered`, and the empty head agrees with every log, asking nothing.
    /// Then this device reads the log's head as the server serves it now, proved to extend the
    /// one it kept (`read_log`): a head of the same size must be that one; for a shorter one
    /// the server answers its root at that size with a proof that the head just read extends it
    /// (`log-page.known-root`). It is a fork when that proved root is not theirs (kept as
    /// evidence with the proof, `LogFork::prefix`, `LogFork::proof`), and when theirs is longer:
    /// this assumes one linearizable server per log, which only grows, so this device, reading
    /// after the other kept its head, holds a head at least as long. A proof that does not
    /// check is `Tampered`, kept as a verdict of its own (`ForkKind::Misbehaved`): the server
    /// proved it misbehaves, though not what it hides. A head the server served this device
    /// that a later read does not reach is a rollback, kept as a fork of its own log
    /// (`HeadFrom::Server`). A verdict is kept, the first per block (`Refs::found_fork`), and
    /// told once (`Event::LogForked`); one found later answers that one. Needs the server:
    /// offline, or for a local account, `ClientError::Disconnected`.
    pub async fn compare_log_head(
        &self,
        block: Uuid,
        theirs: SignedLogHead,
        via: HeadVia,
    ) -> Result<LogAgreement, ClientError> {
        if !self.vouched(block, &theirs) {
            return Err(ClientError::Forged(block));
        }
        if theirs.head.size == 0 {
            return match *theirs.head.root.as_bytes() == merkle::empty_root() {
                true => Ok(LogAgreement::Agrees),
                false => Err(ClientError::Tampered(theirs.head.root)),
            };
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
            HeadVia::Account => HeadFrom::Account {
                signature: theirs.signature.clone(),
            },
        };
        let head = theirs.head;
        // The longest head the server named meanwhile, which every later read must reach.
        let mut named: Option<LogHead> = None;
        for _ in 0..COMPARES_WHILE_GROWING {
            let ours = self.read_log(block, u64::MAX, 1).await?.head;
            if let Some(named) = named.filter(|named| named.size > ours.size) {
                return Err(self.own_fork(block, &named, &ours));
            }
            if head.size >= ours.size {
                return match head == ours {
                    true => Ok(LogAgreement::Agrees),
                    false => self.forked(block, (&ours, &head), (None, Vec::new()), from),
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
                named = Some(page.head);
                continue;
            }
            if page.head != ours {
                // The server serves this device a head that does not extend the one just read.
                return Err(self.own_fork(block, &ours, &page.head));
            }
            let proof: Vec<merkle::TreeHash> = page
                .consistency
                .iter()
                .map(|hash| *hash.as_bytes())
                .collect();
            let Some(prefix) = page.known_root.filter(|prefix| {
                merkle::consistent(
                    head.size,
                    prefix.as_bytes(),
                    ours.size,
                    ours.root.as_bytes(),
                    &proof,
                )
            }) else {
                return Err(self.misbehaved(block, &ours, &head, from));
            };
            return match prefix == head.root {
                true => Ok(LogAgreement::Agrees),
                false => self.forked(
                    block,
                    (&ours, &head),
                    (Some(*prefix.as_bytes()), proof),
                    from,
                ),
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

    /// Trades heads of blocks' signed logs with the account's other devices through the
    /// account's block (guides/blocks/keys.md, "Forks"), so one person's devices compare even
    /// when they share no session or commit: first it compares each head another device shared
    /// for a log this device reads, in this organization, that is not the one it kept
    /// (`compare_log_head`, `HeadVia::Account`); then it shares its own, signed, where the
    /// block holds none or a shorter one, at most `SHARED_HEADS` a pass. Answers the blocks it
    /// compared, each with how it stood; one that could not be compared is left out. Nothing
    /// for a local account or one with no account block.
    pub async fn trade_account_heads(&self) -> Result<Vec<(Uuid, LogAgreement)>, ClientError> {
        if self.local {
            return Ok(Vec::new());
        }
        let connection = self.connection();
        let identity = self.keys.identity().clone();
        let Some(records) =
            crate::AccountRecords::open(&connection, self.account(), &identity).await?
        else {
            return Ok(Vec::new());
        };
        let content = records.read().await?;
        let shared: Vec<(Uuid, SharedHead)> = content
            .root()
            .shared_log_heads()
            .filter_map(|(block, bytes)| Some((block, SharedHead::decode(bytes)?)))
            .filter(|(_, shared)| shared.organization == self.organization)
            .collect();
        let mut compared = Vec::new();
        for (block, held) in &shared {
            let Some(kept) = self.refs.log_head(*block) else {
                continue;
            };
            if (kept.size, kept.root) == (held.size, held.root) {
                continue;
            }
            let theirs = SignedLogHead {
                account: self.account(),
                head: LogHead {
                    size: held.size,
                    root: Hash::from_bytes(held.root),
                },
                signature: held.signature.clone(),
            };
            if let Ok(agreement) = self.compare_log_head(*block, theirs, HeadVia::Account).await {
                compared.push((*block, agreement));
            }
        }
        let held: std::collections::HashMap<Uuid, u64> = shared
            .iter()
            .map(|(block, held)| (*block, held.size))
            .collect();
        let mut sharing: Vec<(Uuid, SharedHead)> = self
            .refs
            .logs_kept()
            .into_iter()
            .filter_map(|block| {
                let signed = self.signed_log_head(block)?;
                let ahead = held
                    .get(&block)
                    .is_none_or(|size| *size < signed.head.size);
                ahead.then(|| {
                    (
                        block,
                        SharedHead {
                            organization: self.organization,
                            size: signed.head.size,
                            root: *signed.head.root.as_bytes(),
                            signature: signed.signature,
                        },
                    )
                })
            })
            .collect();
        sharing.sort_by_key(|(block, _)| *block);
        sharing.truncate(SHARED_HEADS);
        if !sharing.is_empty() {
            records
                .update(move |content| {
                    let root = content.root();
                    let edits = sharing
                        .iter()
                        .filter(|(block, head)| {
                            root.log_head(*block)
                                .and_then(SharedHead::decode)
                                .is_none_or(|held| held.size < head.size)
                        })
                        .map(|(block, head)| {
                            be_block::AccountBlock::set_log_head(*block, Some(head.encode()))
                        })
                        .collect();
                    crate::peer::account_block::answered(edits)
                })
                .await?;
        }
        Ok(compared)
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
        // The newest verified key is the contact's current one, as `verify_authored` holds it;
        // one it replaced, or one a broken chain superseded, vouches for nothing new.
        self.refs
            .signers(signed.account)
            .iter()
            .filter(|known| known.verified)
            .next_back()
            .is_some_and(|current| !current.superseded && verifies(SigningPublic(current.key)))
    }

    /// The server served this device `served`, which does not extend `kept`: kept as the
    /// block's fork (`HeadFrom::Server`), so one place answers whether it forked, and answered
    /// as `ClientError::LogForked`.
    pub(crate) fn own_fork(&self, block: Uuid, kept: &LogHead, served: &LogHead) -> ClientError {
        if let Err(error) = self.forked(block, (kept, served), (None, Vec::new()), HeadFrom::Server)
        {
            return error;
        }
        ClientError::LogForked(block)
    }

    /// The server answered a proof about `block`'s log that does not check: kept as a verdict
    /// of its own (`ForkKind::Misbehaved`), and answered as `ClientError::Tampered`.
    pub(crate) fn misbehaved(
        &self,
        block: Uuid,
        ours: &LogHead,
        theirs: &LogHead,
        from: HeadFrom,
    ) -> ClientError {
        let fork = self.verdict(
            block,
            (ours, theirs),
            (None, Vec::new()),
            from,
            ForkKind::Misbehaved,
        );
        if let Err(error) = self.keep_verdict(fork) {
            return error;
        }
        ClientError::Tampered(ours.root)
    }

    fn forked(
        &self,
        block: Uuid,
        (ours, theirs): (&LogHead, &LogHead),
        evidence: (Option<[u8; 32]>, Vec<merkle::TreeHash>),
        from: HeadFrom,
    ) -> Result<LogAgreement, ClientError> {
        let fork = self.verdict(block, (ours, theirs), evidence, from, ForkKind::Forked);
        let kept = self.keep_verdict(fork)?;
        Ok(LogAgreement::Forked(kept))
    }

    fn verdict(
        &self,
        block: Uuid,
        (ours, theirs): (&LogHead, &LogHead),
        (prefix, proof): (Option<[u8; 32]>, Vec<merkle::TreeHash>),
        from: HeadFrom,
        kind: ForkKind,
    ) -> LogFork {
        let kept = |head: &LogHead| KeptLogHead {
            size: head.size,
            root: *head.root.as_bytes(),
            frontier: None,
        };
        LogFork {
            block,
            ours: kept(ours),
            theirs: kept(theirs),
            prefix,
            proof,
            from,
            kind,
            found: self.server_now(),
        }
    }

    /// Keeps `fork` as its block's verdict unless one is kept already, telling it once:
    /// answers the verdict kept.
    fn keep_verdict(&self, fork: LogFork) -> Result<LogFork, ClientError> {
        if self.refs.found_fork(fork.clone())? {
            self.connection().fork_found(fork.clone());
            return Ok(fork);
        }
        Ok(self.refs.log_fork(fork.block).unwrap_or(fork))
    }
}
