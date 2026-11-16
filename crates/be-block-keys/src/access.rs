//! Signed access records (plans/signed_access.md): each block, and each company organization,
//! has a log of entries signed by the members who made them, which devices check back to the
//! log's root before they act on who may do what.
//!
//! An entry names the hash of the entry before it, so its place in the log orders it. The root
//! is the first entry; a device takes its hash only from somewhere the server cannot write
//! (making the log itself, or a key that carries it, `CarriedKey`), so a server that serves a log
//! rooted elsewhere is caught.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use std::collections::HashMap;

use crate::{KeyError, PublicIdentity, RecoverySecret, SigningPublic, verify_signature};

/// What an access record's signature covers comes after this, so it never verifies as anything
/// else the account's signing key signs (authorship has a domain of its own).
const ACCESS_INFO: &[u8] = b"be3.access.v1";
/// What an entry's hash covers comes after this.
const ACCESS_HASH_INFO: &[u8] = b"be3.access.hash.v1";
/// What an old signing key signs to hand an account's records on to its new keys comes after
/// this (`Retirement::endorsement`).
const RETIREMENT_INFO: &[u8] = b"be3.access.retirement.v1";

/// The hash of an access log entry, by which the next entry and a device's pin name it.
pub type EntryHash = [u8; 32];

/// A record in an access log.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Record {
    /// The first entry of a log: who made the block or organization, with the keys they made it
    /// with.
    Root(Root),
    /// An account took a new recovery phrase: its records after this are signed with `new`
    /// (plans/signed_access.md, "A new recovery phrase").
    Retirement(Retirement),
    /// A member's access to the block, as a member who may give it gave it; revoking is a grant
    /// of `Level::None`.
    Grant(Grant),
    /// Who administers the organization, in its log.
    Administrator(Administrator),
}

/// A member's access to a block, from least to most (`be_block_graph::Access`).
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Level {
    None,
    KnowExists,
    View,
    Edit,
    Manage,
}

/// `grantee`'s access, with the keys the granter verified for them, which a device seals to.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Grant {
    pub grantee: [u8; 16],
    pub identity: PublicIdentity,
    pub level: Level,
}

/// Whether `account`, with the keys the administrator signing verified, administers the
/// organization.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Administrator {
    pub account: [u8; 16],
    pub identity: PublicIdentity,
    pub administrator: bool,
}

/// What a signer holds from outside the log being checked: administering the organization (its
/// log checked), or Manage inherited from a block above.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Outside {
    pub level: Level,
    pub signing: SigningPublic,
}

/// An account's move to new keys, endorsed by the key it retires and signed (as an entry) by
/// the new one: the server cannot retire an account to keys of its own, and the old key signs
/// nothing in the log after it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Retirement {
    pub account: [u8; 16],
    pub old: SigningPublic,
    pub new: PublicIdentity,
    #[serde(with = "serde_bytes")]
    pub endorsement: Vec<u8>,
}

fn endorsed(account: [u8; 16], new: &PublicIdentity) -> Vec<u8> {
    [RETIREMENT_INFO, &account, &new.public.0, &new.signing.0].concat()
}

/// What a log is the log of, so a block's records never verify as an organization's, nor the
/// other way round.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum LogKind {
    Block,
    Organization,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Root {
    pub kind: LogKind,
    pub creator: [u8; 16],
    pub identity: PublicIdentity,
}

/// One signed entry of the log of `log` (a block or organization id).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Entry {
    pub log: [u8; 16],
    pub previous: Option<EntryHash>,
    pub record: Record,
    pub signer: [u8; 16],
    #[serde(with = "serde_bytes")]
    pub signature: Vec<u8>,
}

#[derive(Serialize)]
struct Signed<'a> {
    log: [u8; 16],
    previous: Option<EntryHash>,
    record: &'a Record,
    signer: [u8; 16],
}

fn signed(
    log: [u8; 16],
    previous: Option<EntryHash>,
    record: &Record,
    signer: [u8; 16],
) -> Vec<u8> {
    let signed = Signed {
        log,
        previous,
        record,
        signer,
    };
    let mut bytes = ACCESS_INFO.to_vec();
    bytes.extend(postcard::to_stdvec(&signed).expect("an access record encodes"));
    bytes
}

impl Entry {
    /// `record`, signed by account `signer` with `secret`, at the place after `previous` in the
    /// log of `log`.
    pub fn sign(
        secret: &RecoverySecret,
        signer: [u8; 16],
        log: [u8; 16],
        previous: Option<EntryHash>,
        record: Record,
    ) -> Self {
        use ed25519_dalek::Signer;
        let signature = secret
            .signing_key()
            .sign(&signed(log, previous, &record, signer))
            .to_bytes()
            .to_vec();
        Self {
            log,
            previous,
            record,
            signer,
            signature,
        }
    }

    /// The root of the `kind` log of `log`, made by `creator` with `secret`.
    pub fn root(secret: &RecoverySecret, creator: [u8; 16], kind: LogKind, log: [u8; 16]) -> Self {
        Self::sign(
            secret,
            creator,
            log,
            None,
            Record::Root(Root {
                kind,
                creator,
                identity: secret.identity(),
            }),
        )
    }

    /// The entry after `previous` in the log of `log` that retires `account`'s `old` keys for
    /// `new`'s: endorsed by the old key, signed by the new one.
    pub fn retirement(
        old: &RecoverySecret,
        new: &RecoverySecret,
        account: [u8; 16],
        log: [u8; 16],
        previous: EntryHash,
    ) -> Self {
        use ed25519_dalek::Signer;
        let endorsement = old
            .signing_key()
            .sign(&endorsed(account, &new.identity()))
            .to_bytes()
            .to_vec();
        Self::sign(
            new,
            account,
            log,
            Some(previous),
            Record::Retirement(Retirement {
                account,
                old: old.signing(),
                new: new.identity(),
                endorsement,
            }),
        )
    }

    /// Whether the signature is `signing`'s over this entry.
    pub fn signed_by(&self, signing: &crate::SigningPublic) -> bool {
        verify_signature(
            &signing.0,
            &signed(self.log, self.previous, &self.record, self.signer),
            &self.signature,
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("an access entry encodes")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, KeyError> {
        postcard::from_bytes(bytes).map_err(|_| KeyError::Record)
    }

    pub fn hash(&self) -> EntryHash {
        Sha256::new()
            .chain_update(ACCESS_HASH_INFO)
            .chain_update(self.encode())
            .finalize()
            .into()
    }

    /// The root this entry is, when it is the well-formed, self-signed first entry of the `kind`
    /// log of `log`.
    pub fn checked_root(&self, kind: LogKind, log: [u8; 16]) -> Result<Root, KeyError> {
        let Record::Root(root) = &self.record else {
            return Err(KeyError::Record);
        };
        let well_formed = root.kind == kind
            && self.log == log
            && self.previous.is_none()
            && self.signer == root.creator;
        match well_formed && self.signed_by(&root.identity.signing) {
            true => Ok(*root),
            false => Err(KeyError::Record),
        }
    }
}

/// A key as it travels sealed or wrapped: the key, the block (or, for an escrow key, the
/// organization) it is a key of, and the hash of that log's root when the sender holds it
/// pinned. Sealed together, so a server that relabels a key row as another block's, or serves a
/// block's log rooted elsewhere, is caught by whoever opens it. A sender that holds no pinned
/// root (it reached the block only through escrow, say) sends none, and the key pins nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CarriedKey {
    pub key: [u8; 32],
    pub block: [u8; 16],
    pub root: Option<EntryHash>,
}

impl CarriedKey {
    const BARE: usize = 32 + 16 + 1;

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = [self.key.as_slice(), &self.block].concat();
        match &self.root {
            Some(root) => {
                bytes.push(1);
                bytes.extend_from_slice(root);
            }
            None => bytes.push(0),
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, KeyError> {
        let (head, root) = bytes
            .split_at_checked(Self::BARE)
            .ok_or(KeyError::Corrupt)?;
        let (key, rest) = head.split_at(32);
        let (block, flag) = rest.split_at(16);
        let root = match (flag, root.len()) {
            ([0], 0) => None,
            ([1], 32) => Some(root.try_into().map_err(|_| KeyError::Corrupt)?),
            _ => return Err(KeyError::Corrupt),
        };
        Ok(Self {
            key: key.try_into().map_err(|_| KeyError::Corrupt)?,
            block: block.try_into().map_err(|_| KeyError::Corrupt)?,
            root,
        })
    }
}

/// A log checked from its root to its head (`check_log`): the hash of its head, the signing key
/// each account that holds a record in it signs with now, and what the records give.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Checked {
    pub head: EntryHash,
    pub signers: HashMap<[u8; 16], SigningPublic>,
    /// For a block's log: each account's access as its records give it (the creator's is
    /// Manage), with the keys last named for it.
    pub grants: HashMap<[u8; 16], (Level, PublicIdentity)>,
    /// For an organization's log: its administrators, with their keys.
    pub administrators: HashMap<[u8; 16], PublicIdentity>,
}

/// Checks the `kind` log of `log` from its root, which must be the one `pinned`, to its head:
/// every entry names the hash of the one before and is signed by its signer's current key, and is
/// one its signer may make at its place: a retirement only of the signer's own keys, endorsed by
/// the key it retires; a grant to another account only by a signer with Manage (from the log or
/// from `outside`), and anyone's revocation of their own grant; an administrator record only by
/// an administrator. Anything else makes the whole log refused, since a device acts only on a
/// log it checked to its head. A signer the log names no keys for signs with the keys `outside`
/// gives.
pub fn check_log(
    kind: LogKind,
    log: [u8; 16],
    pinned: EntryHash,
    entries: &[Entry],
    outside: &dyn Fn([u8; 16]) -> Option<Outside>,
) -> Result<Checked, KeyError> {
    let (root, rest) = entries.split_first().ok_or(KeyError::Record)?;
    let checked = root.checked_root(kind, log)?;
    if root.hash() != pinned {
        return Err(KeyError::Record);
    }
    let mut state = Checked {
        head: pinned,
        signers: HashMap::from([(checked.creator, checked.identity.signing)]),
        ..Checked::default()
    };
    match kind {
        LogKind::Block => {
            state
                .grants
                .insert(checked.creator, (Level::Manage, checked.identity));
        }
        LogKind::Organization => {
            state
                .administrators
                .insert(checked.creator, checked.identity);
        }
    }
    for entry in rest {
        let from_outside = outside(entry.signer);
        let current = state
            .signers
            .get(&entry.signer)
            .copied()
            .or(from_outside.map(|outside| outside.signing))
            .ok_or(KeyError::Record)?;
        if entry.log != log || entry.previous != Some(state.head) {
            return Err(KeyError::Record);
        }
        let level = state
            .grants
            .get(&entry.signer)
            .map_or(Level::None, |(level, _)| *level)
            .max(from_outside.map_or(Level::None, |outside| outside.level));
        match &entry.record {
            Record::Root(_) => return Err(KeyError::Record),
            Record::Retirement(retirement) => {
                let endorsed_by_current = retirement.account == entry.signer
                    && retirement.old == current
                    && verify_signature(
                        &current.0,
                        &endorsed(retirement.account, &retirement.new),
                        &retirement.endorsement,
                    );
                if !endorsed_by_current || !entry.signed_by(&retirement.new.signing) {
                    return Err(KeyError::Record);
                }
                state.signers.insert(entry.signer, retirement.new.signing);
            }
            Record::Grant(grant) => {
                let own_revocation = grant.grantee == entry.signer && grant.level == Level::None;
                let may = kind == LogKind::Block && (own_revocation || level == Level::Manage);
                if !may || !entry.signed_by(&current) {
                    return Err(KeyError::Record);
                }
                state.signers.insert(grant.grantee, grant.identity.signing);
                state
                    .grants
                    .insert(grant.grantee, (grant.level, grant.identity));
            }
            Record::Administrator(administrator) => {
                let may = kind == LogKind::Organization
                    && state.administrators.contains_key(&entry.signer);
                if !may || !entry.signed_by(&current) {
                    return Err(KeyError::Record);
                }
                state
                    .signers
                    .insert(administrator.account, administrator.identity.signing);
                match administrator.administrator {
                    true => {
                        state
                            .administrators
                            .insert(administrator.account, administrator.identity);
                    }
                    false => {
                        state.administrators.remove(&administrator.account);
                    }
                }
            }
        }
        state.head = entry.hash();
    }
    Ok(state)
}
